# Crake built by Nix with crane: its crates.io dependencies in a derivation
# of their own (unchanged while Cargo.lock is, so CI keeps it cached), then
# Crake itself on top.
{ lib, craneLib, pkg-config, cmake, libxcb, libxkbcommon, fontconfig
, freetype, wayland, vulkan-loader, libGL }:

let
  commonArgs = {
    pname = "crake";
    version = "0.1.0";
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
  cargoArtifacts = craneLib.buildDepsOnly (removeAttrs commonArgs [ "src" ] // {
    dummySrc = craneLib.mkDummySrc {
      inherit (commonArgs) src;
      # The vendored, patched GPUI X11 backend is a dependency, not Crake's
      # own code: whole, not a dummy (vendor/gpui-pre-linux/CRAKE-PATCH.md).
      extraDummyScript = ''
        rm -rf $out/vendor
        cp -r ${../../vendor} $out/vendor
      '';
    };
  });
in
craneLib.buildPackage (commonArgs // {
  inherit cargoArtifacts;

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
