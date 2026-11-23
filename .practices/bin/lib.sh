# shellcheck shell=bash disable=SC2034 # (settings read by the scripts that source this)
# Sourced by every script in .practices/bin: finds the project's root,
# loads its settings (.practices/config) over the defaults below, and
# gives the helpers the checks share.
#
# Written for bash 3.2 (macOS' /bin/bash) and BSD tools as much as GNU
# ones: no mapfile, no associative arrays, no GNU-only flags. A check that
# can't do its work fails; it never reports "clean" (in Crake until
# 2026-10-26, a privacy check let every commit through on macOS that way).

set -uo pipefail

PRACTICES_BIN=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# The project is the repository this kit sits in (.practices/bin), not
# the one the caller happens to be in. In a git hook, git sets GIT_DIR and
# runs the hook at the top of the tree being committed, which may be a
# linked worktree whose hooks are the main tree's (core.hooksPath): there,
# `git -C .practices/bin rev-parse --show-toplevel` answers .practices/bin
# itself, and every check ran from the wrong folder (Scops, 2026-11-22).
if [ -n "${GIT_DIR:-}" ]; then
    ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || { echo "practices: run this inside the project's git repository" >&2; exit 2; }
elif ! ROOT=$(git -C "$PRACTICES_BIN" rev-parse --show-toplevel 2>/dev/null); then
    echo "practices: run this inside the project's git repository" >&2
    exit 2
fi
cd "$ROOT" || exit 2

# Defaults. A project overrides any of them in .practices/config (shell
# KEY=value lines, sourced here).

# Git pathspecs (space separated) left out of every check: vendored code
# isn't the project's to fix.
EXCLUDE=':!vendor'
# Lines of code per Rust source file, and per integration test file
# (under a tests/ folder). An inline #[cfg(test)] module isn't counted.
SIZE_CODE=700
SIZE_TESTS=1000
# Folders where `unsafe` may appear (space separated); empty: nowhere.
UNSAFE_ALLOWED=''
# The git-ignored list of names that must never be committed (clients,
# their hosts and addresses), one PCRE pattern per line. "none" says the
# project has no such names: the check then passes on purpose.
PRIVATE_TERMS=dev/private-terms
# The disk the build folder is on keeps at least this many GB free.
DISK_FLOOR_GB=300
# Files (a regex on paths relative to the root) left out of the coverage
# share: test harness code that only runs outside `cargo test`.
COVERAGE_EXCLUDE=''
# Files (a regex on paths from the root) the size limit skips: generated
# code nobody edits by hand.
SIZE_EXCLUDE=''
# A JavaScript/TypeScript project (a package.json at the root): the npm
# scripts the lint gate runs after `tsc --noEmit`, and the one the
# pre-commit hook runs when JS/TS files are committed (empty: none).
NPM_CHECKS='lint'
NPM_TEST='test'
# The UI gate (check-ui): the HTML design boards it renders (git
# pathspecs), the widths (px) and themes it renders each at, what counts
# as a separate target and as a state class, and Chrome's path (empty:
# found in the usual places).
UI_BOARDS=':(glob)design/**/*.html'
UI_WIDTHS='1100 1440'
UI_THEMES='light dark'
UI_TARGETS='button, a[href], [role=button], input, select, textarea, summary'
UI_STATES='focus hover active pressed'
UI_CHROME=''
# The page gate (check-page): the proposal and review pages the owner
# reads on a phone (git pathspecs; empty: none), and the widths (px) each
# is read at. Crake keeps its proposals in design/proposals/, Scops its
# in design/changes/.
UI_PAGES=':(glob)design/proposals/**/*.html :(glob)design/changes/**/*.html'
PAGE_WIDTHS='375 390'
# The voice gate (check-voice): the files that hold the copy people see
# (git pathspecs; tests and generated files left out), and more first-person plural words to flag, as a regex
# alternation (a language the check doesn't know). The changelog is What's new
# and the release notes, and a content/ folder holds a FAQ or help pages: until
# 1.6.1 neither was read, and a FAQ's "not even us" passed.
VOICE_FILES=':(glob)CHANGELOG*.md :(glob)**/content/**/*.ts :(glob)**/content/**/*.tsx :(glob)**/content/**/*.js :(glob)**/content/**/*.json :(glob)**/content/**/*.md :(glob)**/i18n/** :(glob)**/locales/** :(glob)**/l10n/** :(glob)**/*.lproj/*.strings :(glob)**/*.xcstrings :(glob)**/*.ftl :(glob)**/*.po :(glob)**/*.arb :(glob)**/res/values*/strings.xml :(glob)fastlane/metadata/**/*.txt :(exclude,glob)**/__tests__/** :(exclude,glob)**/*.test.* :(exclude,glob)**/*.spec.* :(exclude,glob)**/tests/** :(exclude,glob)**/*.generated.*'
VOICE_WORDS=''
# The copy gate (check-copy): the files it reads (git pathspecs; empty: the
# design boards and the copy files, UI_BOARDS and VOICE_FILES), the
# language of copy that names none (empty: such copy is a finding, never a
# guess; check-voice reads it too, for the -me verbs a language carries),
# and the folder of the project's own copy-rules/<lang>.mjs files,
# which add to the kit's (cs, en, es) and add languages.
COPY_FILES=''
COPY_LANG=''
COPY_RULES='.practices/copy-rules'
# The project's own checks (git pathspecs: a lint.d script, a scanner's
# rules). A commit that adds or changes one says, in a "Proven:" line, how
# the check was broken once and its test failed (check-proven,
# common/gates.md). Empty: off. The environment may set it, as the
# practices repository's own commit-msg hook does.
CHECK_FILES=${CHECK_FILES-}
# The permission-button gate (check-permission-words): the code files it
# reads besides the copy files (VOICE_FILES), tests left out, and more
# verbs and permission nouns as regex alternations (a language it doesn't
# know: it knows English, Czech, Spanish and Vietnamese).
PERMISSION_FILES=':(glob)**/*.swift :(glob)**/*.kt :(glob)**/*.java :(glob)**/*.ts :(glob)**/*.tsx :(glob)**/*.js :(glob)**/*.jsx :(glob)**/*.dart :(exclude,glob)**/*Tests/** :(exclude,glob)**/*Tests.swift :(exclude,glob)**/__tests__/** :(exclude,glob)**/*.test.* :(exclude,glob)**/*.spec.* :(exclude,glob)**/tests/** :(exclude,glob)**/test/** :(exclude,glob)**/androidTest/** :(exclude,glob)**/e2e/**'
PERMISSION_VERBS=''
PERMISSION_NOUNS=''
# The Xcode gate (check-xcode, check-ios-artefact): the device families
# the app ships for (TARGETED_DEVICE_FAMILY's form: 1 iPhone, 2 iPad,
# "1,2" both; empty: the project-level setting in the .xcodeproj), and
# more frameworks that need an entitlement, as Framework=entitlement pairs.
DEVICE_FAMILY=''
ENTITLED_FRAMEWORKS=''
# The UI-test gate (check-ui-tests): the launch arguments that pin the
# app's language (a regex; add the app's own switch, "-AcmeLanguage").
UITEST_LANGUAGE='-AppleLanguages|-AppleLocale'

if [ -f .practices/config ]; then
    # shellcheck source=/dev/null
    . .practices/config
fi

has_rust() { [ -f "$ROOT/Cargo.toml" ]; }
has_node() { [ -f "$ROOT/package.json" ]; }

# A GPUI app: Cargo.lock holds a gpui crate (gpui, gpui-kit, gpui-fast,
# gpui-pre…), or a Cargo.toml, tracked or new, names one as a dependency.
uses_gpui() {
    local spec manifest
    has_rust || return 1
    if [ -f "$ROOT/Cargo.lock" ] && grep -Eq '^name = "gpui([-_][A-Za-z0-9_-]+)?"$' "$ROOT/Cargo.lock"; then
        return 0
    fi
    read -r -a spec <<<"$EXCLUDE"
    while IFS= read -r manifest; do
        [ -f "$manifest" ] &&
            grep -Eq '^[[:space:]]*(gpui([-_][A-Za-z0-9_-]+)?[[:space:]]*[=.]|\[([a-z-]+\.)?(dev-|build-)?dependencies\.gpui)' "$manifest" &&
            return 0
    done < <(git ls-files --cached --others --exclude-standard -- 'Cargo.toml' '*/Cargo.toml' ${spec[@]+"${spec[@]}"})
    return 1
}

# The project's Rust files, tracked or new, without the excluded ones.
rust_files() {
    local spec
    read -r -a spec <<<"$EXCLUDE"
    git ls-files --cached --others --exclude-standard -- '*.rs' ${spec[@]+"${spec[@]}"}
}

# The project's JavaScript and TypeScript files, tracked or new, without
# the excluded ones (none in a project without a package.json).
js_files() {
    local spec
    has_node || return 0
    read -r -a spec <<<"$EXCLUDE"
    git ls-files --cached --others --exclude-standard -- \
        '*.ts' '*.tsx' '*.js' '*.jsx' '*.mjs' '*.cjs' ${spec[@]+"${spec[@]}"}
}

# The files check-voice reads: tracked or new, matching VOICE_FILES, and
# the design boards (UI_BOARDS): a board's copy is the product's copy.
# Until 1.7.3 the boards were read for grammar but never for "we".
voice_files() {
    local spec
    {
        if [ -n "$VOICE_FILES" ]; then
            read -r -a spec <<<"$VOICE_FILES $EXCLUDE"
            git ls-files --cached --others --exclude-standard -- "${spec[@]}"
        fi
        [ -z "$UI_BOARDS" ] || ui_boards
    } | awk '!seen[$0]++'
}

# The files check-copy reads: tracked or new, matching COPY_FILES, or the
# boards and the copy files when it is empty.
copy_files() {
    local spec
    if [ -z "$COPY_FILES" ]; then
        { ui_boards; voice_files; } | awk '!seen[$0]++'
        return 0
    fi
    read -r -a spec <<<"$COPY_FILES $EXCLUDE"
    git ls-files --cached --others --exclude-standard -- "${spec[@]}"
}

# The files check-permission-words reads: the copy files (VOICE_FILES,
# without the boards) and the code files in PERMISSION_FILES. Prose (a
# changelog, a FAQ page, store notes) holds no buttons and is left out.
permission_files() {
    local spec
    {
        if [ -n "$VOICE_FILES" ]; then
            read -r -a spec <<<"$VOICE_FILES $EXCLUDE"
            git ls-files --cached --others --exclude-standard -- "${spec[@]}"
        fi
        if [ -n "$PERMISSION_FILES" ]; then
            read -r -a spec <<<"$PERMISSION_FILES $EXCLUDE"
            git ls-files --cached --others --exclude-standard -- "${spec[@]}"
        fi
    } | grep -vE '\.(md|txt|html?)$' | awk '!seen[$0]++'
}

# The files whose change can change what check-xcode and check-ui-tests
# find: Xcode projects, XcodeGen specs, entitlements, Info.plists, Swift
# and Objective-C sources, test plans, scripts and workflows.
xcode_files() {
    local spec
    read -r -a spec <<<"$EXCLUDE"
    git ls-files --cached --others --exclude-standard -- '*.pbxproj' 'project.yml' '*/project.yml' \
        '*.entitlements' '*Info.plist' '*.swift' '*.m' '*.mm' '*.xctestplan' '*.xcconfig' \
        '*.sh' '.github/workflows/*' 'Makefile' 'fastlane/Fastfile' ${spec[@]+"${spec[@]}"} | awk '!seen[$0]++'
}

# The Xcode projects: each tracked or new .xcodeproj, without CocoaPods'.
xcode_projects() {
    local spec
    read -r -a spec <<<"$EXCLUDE"
    git ls-files --cached --others --exclude-standard -- '*.xcodeproj/project.pbxproj' ':!Pods/**' ':!**/Pods/**' \
        ':!**/node_modules/**' ${spec[@]+"${spec[@]}"} | sed 's|/project\.pbxproj$||'
}

# The proposal and review pages check-page reads: tracked or new,
# matching UI_PAGES.
ui_pages() {
    local spec
    [ -n "$UI_PAGES" ] || return 0
    read -r -a spec <<<"$UI_PAGES $EXCLUDE"
    git ls-files --cached --others --exclude-standard -- "${spec[@]}"
}

# The design boards check-ui renders: tracked or new, matching UI_BOARDS.
ui_boards() {
    local spec
    [ -n "$UI_BOARDS" ] || return 0
    read -r -a spec <<<"$UI_BOARDS $EXCLUDE"
    git ls-files --cached --others --exclude-standard -- "${spec[@]}"
}
