# Crake built by Nix with crane: its crates.io dependencies in a derivation
# of their own (unchanged while Cargo.lock is, so CI keeps it cached), then
# Crake itself on top.
{ lib, craneLib, pkg-config, cmake, jq, libxcb, libxkbcommon, fontconfig
, freetype, wayland, vulkan-loader, libGL }:

let
  commonArgs = {
    pname = "crake";
    version = "0.6.1";
    src = lib.fileset.toSource {
      root = ../..;
      fileset = lib.fileset.unions [
        ../../Cargo.toml ../../Cargo.lock ../../crates ../../vendor
        ../../assets ../../packaging ../../LICENSE
      ];
    };
    strictDeps = true;
    cargoExtraArgs = "--locked -p crake -p craked";
    # The tests need namespaces and containers (scripts/netns-test.sh, CI).
    doCheck = false;
    nativeBuildInputs = [ pkg-config cmake ];
    buildInputs = [ libxcb libxkbcommon fontconfig freetype wayland ];
  };
  # The dependencies don't change with Crake's version, so their builds
  # mustn't see it: a release (which bumps it) then reuses them instead of
  # rebuilding for ~45 min (scripts/nix-deps-check.sh, in CI, says if that
  # breaks). A derivation is hashed by its inputs, so the version is taken
  # out before any is made: the cleaned Cargo.tomls leave it out (Cargo then
  # says 0.0.0), and the dummy source gets a Cargo.lock that says so too.
  depsVersion = "0.0.0";
  lockText = builtins.readFile ../../Cargo.lock;
  crakeVersion = (builtins.fromTOML (builtins.readFile ../../Cargo.toml)).workspace.package.version;
  # Crake's own crates: the entries without a source, at Crake's version
  # (the vendored GPUI crate keeps its own).
  own = builtins.filter (p: !(p ? source) && p.version == crakeVersion)
    (builtins.fromTOML lockText).package;
  entry = v: p: "name = \"${p.name}\"\nversion = \"${v}\"";
  depsLock = builtins.toFile "Cargo.lock"
    (builtins.replaceStrings (map (entry crakeVersion) own) (map (entry depsVersion) own) lockText);
  dummySrc = craneLib.mkDummySrc {
    src = lib.fileset.toSource {
      root = ../..;
      # Not Cargo.lock: it says Crake's version (depsLock below doesn't).
      fileset = lib.fileset.unions [ ../../Cargo.toml ../../crates ../../vendor ];
    };
    cargoLock = depsLock;
    cleanCargoTomlFilter = path:
      path != [ "package" "version" ] && path != [ "workspace" "package" "version" ]
      && craneLib.filters.cargoTomlDefault path;
    # The vendored, patched GPUI X11 backend is a dependency, not Crake's
    # own code: whole, not a dummy (vendor/gpui-pre-linux/CRAKE-PATCH.md).
    extraDummyScript = ''
      rm -rf $out/vendor
      cp -r ${../../vendor} $out/vendor
    '';
  };
  cargoArtifacts = craneLib.buildDepsOnly (removeAttrs commonArgs [ "src" ] // {
    inherit dummySrc;
    version = depsVersion;
  });

  # The journey's test binary (crates/crake/src/journey_tests.rs), for the
  # NixOS VM test: the app crate's tests, built and not run, on dependencies
  # of their own (the tests turn on gpui-kit's test-support).
  journeyArgs = commonArgs // {
    pname = "crake-journey";
    cargoExtraArgs = "--locked -p crake";
  };
  journey = craneLib.mkCargoDerivation (journeyArgs // {
    cargoArtifacts = craneLib.buildDepsOnly (removeAttrs journeyArgs [ "src" ] // {
      inherit dummySrc;
      version = depsVersion;
      doCheck = true;
      cargoTestExtraArgs = "--no-run";
    });
    nativeBuildInputs = journeyArgs.nativeBuildInputs ++ [ jq ];
    buildPhaseCargoCommand = ''
      cargoWithProfile test --locked -p crake --no-run --message-format json-render-diagnostics > build.json
    '';
    doInstallCargoArtifacts = false;
    installPhaseCommand = ''
      mkdir -p $out/bin
      cp "$(jq -r 'select(.reason == "compiler-artifact" and .profile.test and .target.kind == ["bin"]) | .executable' build.json | head -n1)" $out/bin/crake-journey
    '';
  });
in
craneLib.buildPackage (commonArgs // {
  inherit cargoArtifacts;
  passthru = { inherit journey; };

  postInstall = ''
    install -Dm755 $out/bin/crake-hook $out/libexec/crake/hook
    rm $out/bin/crake-hook
    install -Dm644 packaging/software.dorhawk.Crake1.conf $out/share/dbus-1/system.d/software.dorhawk.Crake1.conf
    install -Dm644 packaging/software.dorhawk.crake.policy $out/share/polkit-1/actions/software.dorhawk.crake.policy
    install -Dm644 packaging/deb/crake.desktop $out/share/applications/software.dorhawk.crake.desktop
    install -Dm644 assets/icons/app/crake.svg $out/share/icons/hicolor/scalable/apps/software.dorhawk.crake.svg
    for s in 16 24 32 48 64 128 256 512; do
      install -Dm644 assets/icons/app/crake-$s.png $out/share/icons/hicolor/''${s}x$s/apps/software.dorhawk.crake.png
    done
    install -Dm644 packaging/craked.toml.example $out/share/doc/crake/craked.toml.example
  '';
  # GPUI opens Vulkan, Wayland and libxkbcommon at run time.
  postFixup = ''
    patchelf --add-rpath ${lib.makeLibraryPath [ vulkan-loader wayland libxkbcommon libGL ]} $out/bin/crake
  '';

  meta = {
    description = "VPN client that routes only what needs the tunnel";
    homepage = "https://dorhawk.software/crake/";
    license = with lib.licenses; [ gpl3Plus ofl ];
    mainProgram = "crake";
    platforms = [ "x86_64-linux" ];
  };
})
