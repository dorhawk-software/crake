# Crake built by Nix with crane: its crates.io dependencies in derivations
# of their own, then Crake itself on top. The dependencies come in two
# layers: those of an earlier commit (`base`, flake input crake-base: built
# once, kept in Cachix), and the current ones built on top of them, so a
# changed Cargo.lock recompiles only the crates that changed and what
# depends on them, not all of them (~45 min in CI before).
{ lib, craneLib, pkg-config, cmake, jq, libxcb, libxkbcommon, fontconfig
, freetype, wayland, vulkan-loader, libGL, base }:

let
  commonArgs = {
    pname = "crake";
    version = "0.13.1";
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
    # Cargo keeps a build only while its sources' path stays the same, and
    # crane's vendored sources are a store path named after the whole
    # Cargo.lock: the layer on top of the base (below) would rebuild every
    # crate ("PathToSourceChanged"). So every build reads them through a
    # link at the same place in its build folder (/build in Nix's sandbox).
    # Only where the build folder itself stays put: /build, in Nix's
    # sandbox (CI turns it on). Without one (Nix 2.30 and later name each
    # build's folder anew) the link would move every time and nothing
    # would be reused; the store path at least stays while Cargo.lock does.
    postConfigure = ''
      if [ "$NIX_BUILD_TOP" = /build ]; then
        ln -s "$cargoVendorDir" "$NIX_BUILD_TOP/vendor-cargo-deps"
        sed -i "s|$cargoVendorDir|$NIX_BUILD_TOP/vendor-cargo-deps|g" "$CARGO_HOME/config.toml"
      fi
    '';
  };
  # The dependencies don't change with Crake's version, so their builds
  # mustn't see it: a release (which bumps it) then reuses them instead of
  # rebuilding for ~45 min (scripts/nix-deps-check.sh, in CI, says if that
  # breaks). A derivation is hashed by its inputs, so the version is taken
  # out before any is made: the cleaned Cargo.tomls leave it out (Cargo then
  # says 0.0.0), and the dummy source gets a Cargo.lock that says so too.
  depsVersion = "0.0.0";
  # A tree's dependencies only (crane's dummy source): its Cargo.tomls
  # without the version, its Cargo.lock with Crake's own crates at 0.0.0
  # (the entries without a source, at Crake's version; the vendored GPUI
  # crate keeps its own), and the vendored GPUI X11 backend whole: it's a
  # dependency, not Crake's own code (vendor/gpui-pre-linux/CRAKE-PATCH.md).
  # Built in a directory of the same name whatever the tree, so the layer
  # on top finds the base's builds of it.
  # `src` is the tree's files (the base, a flake input, comes whole: only
  # its Cargo files are kept anyway).
  dummyFor = root: src:
    let
      # A path stays a path: "${root}/…" would copy the whole tree to the
      # store, and the dependencies would change with any file in it.
      file = name: if builtins.isPath root then root + "/${name}" else "${root}/${name}";
      lockText = builtins.readFile (file "Cargo.lock");
      version = (builtins.fromTOML (builtins.readFile (file "Cargo.toml"))).workspace.package.version;
      own = builtins.filter (p: !(p ? source) && p.version == version)
        (builtins.fromTOML lockText).package;
      entry = v: p: "name = \"${p.name}\"\nversion = \"${v}\"";
    in craneLib.mkDummySrc {
      inherit src;
      cargoLock = builtins.toFile "Cargo.lock"
        (builtins.replaceStrings (map (entry version) own) (map (entry depsVersion) own) lockText);
      cleanCargoTomlFilter = path:
        path != [ "package" "version" ] && path != [ "workspace" "package" "version" ]
        && craneLib.filters.cargoTomlDefault path;
      extraDummyScript = ''
        rm -rf $out/vendor
        cp -r ${file "vendor"} $out/vendor
      '';
    };
  dummySrc = dummyFor ../.. (lib.fileset.toSource {
    root = ../..;
    # Not Cargo.lock: it says Crake's version (dummyFor's doesn't).
    fileset = lib.fileset.unions [ ../../Cargo.toml ../../crates ../../vendor ];
  });
  # The dependencies, in two layers: the base's from scratch (crane's
  # buildDepsOnly), then the current ones on top of them (the same cargo
  # commands, crane's buildDepsOnly can't start from earlier artifacts).
  # `cargo check` too, as buildDepsOnly runs it.
  depsLayers = args:
    let
      baseDeps = craneLib.buildDepsOnly (removeAttrs args [ "src" ] // {
        pname = "${args.pname}-base";
        dummySrc = dummyFor base base;
        version = depsVersion;
      });
      doCheck = args.doCheck or true;
    in craneLib.mkCargoDerivation (removeAttrs args [ "src" "cargoTestExtraArgs" ] // {
      src = dummySrc;
      pnameSuffix = "-deps";
      version = depsVersion;
      cargoArtifacts = baseDeps;
      cargoVendorDir = craneLib.vendorCargoDeps { src = dummySrc; };
      env = (args.env or { }) // { CRANE_BUILD_DEPS_ONLY = 1; };
      buildPhaseCargoCommand = ''
        cargoWithProfile check ${args.cargoExtraArgs} ${lib.optionalString doCheck "--all-targets"}
        cargoWithProfile build ${args.cargoExtraArgs}
      '';
      checkPhaseCargoCommand = ''
        cargoWithProfile test ${args.cargoExtraArgs} ${args.cargoTestExtraArgs or "--no-run"}
      '';
      doInstallCargoArtifacts = true;
    });
  cargoArtifacts = depsLayers commonArgs;

  # The journey's test binary (crates/crake/src/journey_tests.rs), for the
  # NixOS VM test: the app crate's tests, built and not run, on dependencies
  # of their own (the tests turn on gpui-kit's test-support).
  journeyArgs = commonArgs // {
    pname = "crake-journey";
    cargoExtraArgs = "--locked -p crake";
  };
  journey = craneLib.mkCargoDerivation (journeyArgs // {
    cargoArtifacts = depsLayers (journeyArgs // {
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
