# Crake built by Nix from Cargo.lock (every crate from crates.io).
{ lib, rustPlatform, pkg-config, cmake, libxcb, libxkbcommon, fontconfig
, freetype, wayland, vulkan-loader, libGL }:

rustPlatform.buildRustPackage {
  pname = "crake";
  version = "0.1.0";

  src = lib.fileset.toSource {
    root = ../..;
    fileset = lib.fileset.unions [
      ../../Cargo.toml ../../Cargo.lock ../../crates ../../vendor
      ../../assets ../../packaging ../../LICENSE
    ];
  };
  cargoLock.lockFile = ../../Cargo.lock;
  cargoBuildFlags = [ "-p" "crake" "-p" "craked" ];
  # The tests need namespaces and containers (scripts/netns-test.sh, CI).
  doCheck = false;

  nativeBuildInputs = [ pkg-config cmake ];
  buildInputs = [ libxcb libxkbcommon fontconfig freetype wayland ];

  postInstall = ''
    install -Dm755 $out/bin/crake-hook $out/libexec/crake/hook
    rm $out/bin/crake-hook
    install -Dm644 packaging/software.dorhawk.Crake1.conf $out/share/dbus-1/system.d/software.dorhawk.Crake1.conf
    install -Dm644 packaging/software.dorhawk.crake.policy $out/share/polkit-1/actions/software.dorhawk.crake.policy
    install -Dm644 packaging/deb/crake.desktop $out/share/applications/software.dorhawk.crake.desktop
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
}
