{
  description = "Crake: a VPN client for Linux that routes only what needs the tunnel";

  # GPUI needs a recent Rust (1.95+): nixos-unstable has it.
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      packages.${system} = rec {
        crake = pkgs.callPackage ./packaging/nix/package.nix { };
        default = crake;
      };
      # services.crake.enable = true;
      nixosModules.default = import ./packaging/nix/module.nix self;
      # A gateway (ocserv) and a client with the module, on their own
      # network: nix build .#checks.x86_64-linux.vm (scripts/nix-test.sh).
      checks.${system}.vm = pkgs.testers.runNixOSTest (import ./packaging/nix/test.nix self);
    };
}
