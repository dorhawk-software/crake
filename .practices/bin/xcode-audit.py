#!/usr/bin/env python3
"""check-xcode's and check-ios-artefact's reading half (mobile/app-review.md).

    xcode-audit.py project [--family F] [--frameworks PAIRS] XCODEPROJ...
    xcode-audit.py artefact [--family F] [--frameworks PAIRS] [--project XCODEPROJ] PATH

project: reads each .xcodeproj's project.pbxproj (no Xcode needed, so it
runs on Linux too). For every target that ships (an app or an extension
for iOS), in every build configuration:
  - TARGETED_DEVICE_FAMILY is the family the app declares (--family, or
    the project-level setting);
  - each framework its sources import, or its Frameworks phase links,
    that needs an entitlement (FamilyControls...) has it in the target's
    CODE_SIGN_ENTITLEMENTS file.
artefact: reads a built .ipa, .app or .xcarchive. For the app and each
extension inside: UIDeviceFamily in its Info.plist is the declared
family, and every framework its executable links, directly or through an
embedded framework (otool -L), has its entitlement in the bundle's
signature (codesign -d --entitlements). $OTOOL and $CODESIGN name the
tools.

Exit 0 clean, 1 findings, 2 it couldn't do its work.
"""
import os
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

# A framework whose use App Review (or the OS) refuses without the entitlement.
# Only mappings known for certain; a project adds its own (ENTITLED_FRAMEWORKS).
NEEDS = {
    "FamilyControls": "com.apple.developer.family-controls",
    "ManagedSettings": "com.apple.developer.family-controls",
    "DeviceActivity": "com.apple.developer.family-controls",
    "HealthKit": "com.apple.developer.healthkit",
    "HomeKit": "com.apple.developer.homekit",
}
FAMILIES = {1: "iPhone", 2: "iPad", 3: "Apple TV", 4: "Apple Watch", 6: "Mac Catalyst", 7: "Apple Vision"}


# --- project.pbxproj: an old-style (OpenStep) property list.

TOKEN = re.compile(r'\s+|/\*.*?\*/|//[^\n]*|"(?:[^"\\]|\\.)*"|[{}()=;,]|[^\s{}()=;,"]+', re.S)


def parse_openstep(text):
    tokens = [t for t in TOKEN.findall(text) if t.strip() and not t.startswith(("/*", "//"))]
    pos = 0

    def value():
        nonlocal pos
        t = tokens[pos]
        pos += 1
        if t == "{":
            d = {}
            while tokens[pos] != "}":
                k = value()
                if tokens[pos] != "=":
                    raise ValueError(f"expected = after {k!r}")
                pos += 1
                d[k] = value()
                if tokens[pos] == ";":
                    pos += 1
            pos += 1
            return d
        if t == "(":
            a = []
            while tokens[pos] != ")":
                a.append(value())
                if tokens[pos] == ",":
                    pos += 1
            pos += 1
            return a
        if t.startswith('"'):
            return re.sub(r'\\(.)', lambda m: {"n": "\n", "t": "\t"}.get(m.group(1), m.group(1)), t[1:-1])
        return t

    return value()


def families(v):
    """"1,2" or 1 -> {1, 2}; None when unset or not a plain list of numbers."""
    if v is None:
        return None
    if isinstance(v, list):
        v = ",".join(str(x) for x in v)
    parts = [p.strip() for p in str(v).split(",") if p.strip()]
    if not parts or not all(p.isdigit() for p in parts):
        return None
    return {int(p) for p in parts}


def say(fs):
    return ",".join(str(f) for f in sorted(fs)) + " (" + ", ".join(FAMILIES.get(f, str(f)) for f in sorted(fs)) + ")"


def xcconfig(path, key):
    try:
        for line in open(path, encoding="utf-8"):
            m = re.match(r"\s*" + re.escape(key) + r"\s*=\s*(.*?)\s*(?://.*)?$", line)
            if m:
                return m.group(1).replace("$(inherited)", "").strip() or None
    except OSError:
        pass
    return None


class Project:
    def __init__(self, xcodeproj):
        self.path = xcodeproj
        self.root = os.path.dirname(os.path.abspath(xcodeproj))
        data = parse_openstep(open(os.path.join(xcodeproj, "project.pbxproj"), encoding="utf-8").read())
        self.o = data["objects"]
        self.project = self.o[data["rootObject"]]
        self.parent = {}
        for k, obj in self.o.items():
            for c in obj.get("children", []) if isinstance(obj, dict) else []:
                self.parent[c] = k

    def file_path(self, ref):
        """A file or group reference's path on disk, or None (SDK or built products)."""
        obj = self.o.get(ref, {})
        tree, path = obj.get("sourceTree", "<group>"), obj.get("path", "")
        if tree == "<absolute>":
            return path
        if tree == "SOURCE_ROOT":
            return os.path.join(self.root, path)
        if tree != "<group>":
            return None
        up = self.parent.get(ref)
        base = self.file_path(up) if up else os.path.join(self.root, self.project.get("projectDirPath", ""))
        return None if base is None else os.path.normpath(os.path.join(base, path))

    def configs(self, owner):
        lst = self.o[owner["buildConfigurationList"]]
        return {self.o[c]["name"]: self.o[c] for c in lst["buildConfigurations"]}

    def setting(self, target, config, key):
        """A build setting as Xcode resolves the levels: target, its xcconfig, project, its xcconfig."""
        levels = [self.configs(target).get(config), self.configs(self.project).get(config)]
        for c in levels:
            if not c:
                continue
            v = c.get("buildSettings", {}).get(key)
            if v is not None and v != "$(inherited)":
                return v
            base = c.get("baseConfigurationReference")
            if base:
                p = self.file_path(base)
                v = xcconfig(p, key) if p else None
                if v is not None:
                    return v
        return None

    def targets(self):
        for ref in self.project.get("targets", []):
            t = self.o[ref]
            if t.get("isa") != "PBXNativeTarget":
                continue
            kind = t.get("productType", "")
            if "bundle." in kind or not ("application" in kind or "extension" in kind):
                continue  # tests, frameworks, tools: they ship inside an app or not at all
            yield t

    def sources(self, target):
        """The source files the target compiles, and the frameworks it links by name."""
        files, linked = [], set()
        for ph in target.get("buildPhases", []):
            phase = self.o.get(ph, {})
            for bf in phase.get("files", []):
                ref = self.o.get(bf, {}).get("fileRef")
                if not ref:
                    continue
                if phase.get("isa") == "PBXFrameworksBuildPhase":
                    name = self.o.get(ref, {}).get("name") or os.path.basename(self.o.get(ref, {}).get("path", ""))
                    if name.endswith(".framework"):
                        linked.add(name[:-len(".framework")])
                elif phase.get("isa") == "PBXSourcesBuildPhase":
                    p = self.file_path(ref)
                    if p:
                        files.append(p)
        for g in target.get("fileSystemSynchronizedGroups", []):  # Xcode 16's folders
            top = self.file_path(g)
            skip = set()
            for ex in self.o.get(g, {}).get("exceptions", []):
                e = self.o.get(ex, {})
                if e.get("target") in (None, self.ref_of(target)):
                    skip.update(os.path.normpath(os.path.join(top, m)) for m in e.get("membershipExceptions", []))
            for d, _, names in os.walk(top or "/nonexistent"):
                for n in names:
                    p = os.path.normpath(os.path.join(d, n))
                    if n.endswith((".swift", ".m", ".mm")) and p not in skip:
                        files.append(p)
        return files, linked

    def ref_of(self, target):
        for k, v in self.o.items():
            if v is target:
                return k
        return None


IMPORT = re.compile(r"^\s*(?:@\w+(?:\([^)]*\))?\s+)*import\s+(?:(?:class|struct|enum|protocol|func|var|let|typealias)\s+)?(\w+)"
                    r"|^\s*@import\s+(\w+)|^\s*#\s*import\s+<(\w+)/", re.M)


def imports(path):
    try:
        text = open(path, encoding="utf-8", errors="replace").read()
    except OSError:
        return set()
    return {next(g for g in m.groups() if g) for m in IMPORT.finditer(text)}


def entitled(ents, key):
    v = ents.get(key)
    return v is True or (isinstance(v, (list, str)) and len(v) > 0)


def rel(p):
    try:
        r = os.path.relpath(p)
        return p if r.startswith("..") else r
    except ValueError:
        return p


def audit_project(xcodeproj, family, needs, findings):
    proj = Project(xcodeproj)
    want = family
    if want is None:
        cfgs = proj.configs(proj.project)
        found = {frozenset(f) for f in (families(c.get("buildSettings", {}).get("TARGETED_DEVICE_FAMILY")) for c in cfgs.values()) if f}
        if len(found) == 1:
            want = set(next(iter(found)))
    shipped = list(proj.targets())
    if want is None and shipped:
        findings.append((rel(xcodeproj), "the project doesn't say which devices the app is for: set TARGETED_DEVICE_FAMILY at the project level, or DEVICE_FAMILY in .practices/config (1 iPhone, 2 iPad, \"1,2\" both)."))
    for t in shipped:
        name = t.get("name", "?")
        configs = list(proj.configs(t))
        sdk = proj.setting(t, configs[0] if configs else "", "SDKROOT")
        if sdk and sdk not in ("iphoneos", "auto"):
            continue  # a Mac, Watch or TV target: its families are its own
        if want is not None:
            by = {}  # what each configuration builds for -> its configurations
            for config in configs:
                got = families(proj.setting(t, config, "TARGETED_DEVICE_FAMILY"))
                if got != want:
                    by.setdefault(frozenset(got) if got else None, []).append(config)
            fix = ",".join(map(str, sorted(want)))
            for got, cs in by.items():
                if got is None:
                    findings.append((rel(xcodeproj), f"target {name} ({', '.join(cs)}) has no TARGETED_DEVICE_FAMILY this check can read; set it to \"{fix}\" on the target."))
                else:
                    findings.append((rel(xcodeproj), f"target {name} ({', '.join(cs)}) builds for {say(got)}, and the app is for {say(want)}. Set TARGETED_DEVICE_FAMILY = \"{fix}\" on the target (XcodeGen's iOS preset sets \"1,2\" on each target, over the project's line)."))
        files, linked = proj.sources(t)
        used = {}
        for f in files:
            for fw in imports(f):
                used.setdefault(fw, rel(f))
        for fw in linked:
            used.setdefault(fw, "its Frameworks phase")
        lacking = {}  # (entitlements file, key) -> configurations
        for config in configs:
            ent_path = proj.setting(t, config, "CODE_SIGN_ENTITLEMENTS")
            ents = {}
            if ent_path:
                full = os.path.join(proj.root, ent_path.replace("$(SRCROOT)/", "").replace("$(PROJECT_DIR)/", ""))
                try:
                    with open(full, "rb") as fh:
                        ents = plistlib.load(fh)
                except (OSError, plistlib.InvalidFileException, ValueError) as e:
                    findings.append((rel(xcodeproj), f"target {name} ({config}): can't read its entitlements {ent_path}: {e}"))
                    continue
            for key in sorted({needs[fw] for fw in used if fw in needs}):
                if not entitled(ents, key):
                    lacking.setdefault((ent_path, key), []).append(config)
        for (ent_path, key), cs in lacking.items():
            fws = sorted(fw for fw in used if needs.get(fw) == key)
            where = ent_path or "it has no entitlements file (CODE_SIGN_ENTITLEMENTS)"
            uses = ", ".join(f"{fw} ({used[fw]})" for fw in fws)
            findings.append((rel(xcodeproj), f"target {name} ({', '.join(cs)}) uses {uses}, and {where} lacks {key}. Add it, or keep {' and '.join(fws)} out of this target's sources."))


# --- A built artefact.

def tool(env, default):
    return os.environ.get(env) or default


def run(cmd):
    p = subprocess.run(cmd, capture_output=True)
    return p.returncode, p.stdout, p.stderr.decode("utf-8", "replace")


def executable(bundle):
    with open(os.path.join(bundle, "Info.plist"), "rb") as fh:
        info = plistlib.load(fh)
    return info, os.path.join(bundle, info.get("CFBundleExecutable", os.path.splitext(os.path.basename(bundle))[0]))


def links(binary, otool):
    code, out, err = run([otool, "-L", binary])
    if code != 0:
        raise RuntimeError(f"{otool} -L {binary}: {err.strip() or 'failed'}")
    return [line.strip().split(" (")[0] for line in out.decode("utf-8", "replace").splitlines()[1:]
            if line.startswith(("\t", " ")) and not line.strip().startswith("(")]


def linked_frameworks(binary, bundle, app, otool, seen=None):
    """The frameworks a binary loads: directly, and through the app's or bundle's embedded frameworks."""
    seen = set() if seen is None else seen
    out = set()
    for lib in links(binary, otool):
        m = re.search(r"(?:^|/)(\w+)\.framework/", lib)
        if not m:
            continue
        out.add(m.group(1))
        if lib.startswith(("@rpath/", "@executable_path/", "@loader_path/")):
            for base in (bundle, app):
                emb = os.path.join(base, "Frameworks", f"{m.group(1)}.framework", m.group(1))
                if os.path.isfile(emb) and emb not in seen:
                    seen.add(emb)
                    out |= linked_frameworks(emb, bundle, app, otool, seen)
                    break
    return out


def entitlements(bundle, codesign):
    code, out, err = run([codesign, "-d", "--entitlements", "-", "--xml", bundle])
    if code != 0:
        raise RuntimeError(f"{codesign} -d --entitlements {bundle}: {err.strip() or 'failed'}")
    at = out.find(b"<?xml")
    if at < 0:
        return {}
    return plistlib.loads(out[at:])


def bundles(app):
    yield app
    for sub in ("PlugIns", "Extensions", "AppClips"):
        d = os.path.join(app, sub)
        if os.path.isdir(d):
            for n in sorted(os.listdir(d)):
                if n.endswith((".appex", ".app")):
                    yield os.path.join(d, n)


def audit_artefact(path, family, needs, findings):
    otool, codesign = tool("OTOOL", "otool"), tool("CODESIGN", "codesign")
    for t in (otool, codesign):
        if not shutil.which(t):
            raise RuntimeError(f"needs {t} (Xcode's command line tools, on macOS), and there is none")
    tmp = None
    try:
        if path.endswith(".ipa"):
            tmp = tempfile.mkdtemp()
            with zipfile.ZipFile(path) as z:
                z.extractall(tmp)
            for n in z.namelist():
                if n.startswith("Payload/") and n.endswith(".app/Info.plist") and n.count("/") == 2:
                    app = os.path.join(tmp, n[:-len("/Info.plist")])
                    break
            else:
                raise RuntimeError(f"{path} has no Payload/*.app")
        elif path.endswith(".xcarchive"):
            apps = os.path.join(path, "Products", "Applications")
            found = [os.path.join(apps, n) for n in sorted(os.listdir(apps))] if os.path.isdir(apps) else []
            if not found:
                raise RuntimeError(f"{path} has no Products/Applications/*.app")
            app = found[0]
        elif path.endswith(".app") and os.path.isdir(path):
            app = path
        else:
            raise RuntimeError(f"{path} is no .ipa, .app or .xcarchive")
        # Os.chmod not needed: zipfile keeps no modes, and nothing here runs the binaries.
        info, _ = executable(app)
        want = family if family is not None else families(info.get("UIDeviceFamily"))
        for b in bundles(app):
            name = os.path.basename(b)
            info, binary = executable(b)
            got = families(info.get("UIDeviceFamily"))
            if want is not None and got is not None and got != want:
                findings.append((name, f"UIDeviceFamily is {say(got)}, and the app is for {say(want)}. Set TARGETED_DEVICE_FAMILY = \"{','.join(map(str, sorted(want)))}\" on its target and build again."))
            elif got is None and b == app:
                findings.append((name, "has no UIDeviceFamily in its Info.plist, so the store can't tell which devices it is for."))
            if not os.path.isfile(binary):
                raise RuntimeError(f"{name}: no executable {os.path.basename(binary)}")
            used = linked_frameworks(binary, b, app, otool)
            ents = entitlements(b, codesign)
            for fw in sorted(used):
                key = needs.get(fw)
                if key and not entitled(ents, key):
                    findings.append((name, f"links {fw}, and its signature lacks {key}: App Review's automated check stops the build. Add the entitlement to this target, or keep {fw} out of what it compiles and links."))
    finally:
        if tmp:
            shutil.rmtree(tmp, ignore_errors=True)


def main(argv):
    mode, argv = (argv[0], argv[1:]) if argv else ("", [])
    family, needs, project = None, dict(NEEDS), None
    while argv[:1] in (["--family"], ["--frameworks"], ["--project"]):
        if argv[0] == "--family" and argv[1]:
            family = families(argv[1])
            if family is None:
                print(f"{'check-xcode' if mode == 'project' else 'check-ios-artefact'}: DEVICE_FAMILY \"{argv[1]}\" isn't a list of numbers (1 iPhone, 2 iPad, \"1,2\" both).")
                return 2
        elif argv[0] == "--frameworks":
            for pair in argv[1].split():
                fw, _, key = pair.partition("=")
                if fw and key:
                    needs[fw] = key
        elif argv[0] == "--project" and argv[1]:
            project = argv[1]
        argv = argv[2:]
    name = "check-xcode" if mode == "project" else "check-ios-artefact"
    if mode not in ("project", "artefact") or not argv:
        print(__doc__)
        return 2
    findings = []
    try:
        if mode == "project":
            for p in argv:
                audit_project(p, family, needs, findings)
        else:
            if family is None and project:
                proj = Project(project)
                found = {frozenset(f) for f in (families(c.get("buildSettings", {}).get("TARGETED_DEVICE_FAMILY")) for c in proj.configs(proj.project).values()) if f}
                family = set(next(iter(found))) if len(found) == 1 else None
            audit_artefact(argv[0], family, needs, findings)
    except (OSError, KeyError, IndexError, ValueError, RuntimeError, zipfile.BadZipFile, plistlib.InvalidFileException) as e:
        print(f"{name} couldn't do its work, so it can't say the app is clean: {e}")
        return 2
    if findings:
        n = len(findings)
        print(f"[app-review] {n} finding{'s' if n > 1 else ''}. Rules: every binary carries the entitlements its frameworks need; the app is built for the devices it declares (.practices/docs/mobile/app-review.md).")
        print("  Why: an iPhone app's widget compiled a shared folder that linked ManagedSettings without Family Controls, and App Review's automated check stopped it (13 Nov 2026). An earlier build shipped for iPad too, and the store asked for 13-inch iPad screenshots.")
        for where, msg in findings:
            print(f"  - {where}: {msg}")
    print(f"\n{name}: {len(argv)} {'project' if mode == 'project' else 'artefact'}{'s' if len(argv) != 1 else ''}: {n if findings else 'no'} finding{'s' if len(findings) != 1 else ''}.")
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
