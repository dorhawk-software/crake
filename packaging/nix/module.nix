# NixOS: services.crake.enable = true; (the flake's nixosModules.default)
self:
{ config, lib, pkgs, ... }:
let
  cfg = config.services.crake;
in {
  options.services.crake = {
    enable = lib.mkEnableOption "Crake, a VPN client that routes only what needs the tunnel";
    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      description = "The Crake package.";
    };
    extraConfig = lib.mkOption {
      type = lib.types.lines;
      default = "";
      description = "More of /etc/crake/craked.toml (see the package's craked.toml.example).";
    };
  };

  config = lib.mkIf cfg.enable {
    # The app signs in with openconnect as the user.
    environment.systemPackages = [ cfg.package pkgs.openconnect ];
    services.dbus.packages = [ cfg.package ];
    security.polkit.enable = true;
    # Tunnel DNS goes through systemd-resolved.
    services.resolved.enable = true;
    # Crake's tunnels are craked's: dhcpcd took crake-acme as its own and
    # the DNS craked handed resolved for it was gone again.
    networking.dhcpcd.denyInterfaces = [ "crake-*" ];
    networking.networkmanager.unmanaged = [ "interface-name:crake-*" ];

    environment.etc."crake/craked.toml".text = ''
      hook_path = "${cfg.package}/libexec/crake/hook"
    '' + cfg.extraConfig;

    # As packaging/craked.service, with the tools craked runs on its path.
    systemd.services.craked = {
      description = "Crake daemon";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" "dbus.service" ];
      wants = [ "network-online.target" ];
      path = [ pkgs.nftables pkgs.iproute2 pkgs.openconnect ];
      # An upgrade (nixos-rebuild switch) reloads craked rather than
      # restarting it: the reload leaves the handover flag and restarts it,
      # so its tunnels stay signed in, as the packages' upgrade scripts do
      # (manager/handover.rs).
      reloadIfChanged = true;
      serviceConfig = {
        ExecReload = "${pkgs.bash}/bin/sh -c 'touch /run/crake/handover && ${pkgs.systemd}/bin/systemctl restart --no-block craked.service'";
        Type = "dbus";
        BusName = "software.dorhawk.Crake1";
        ExecStart = "${cfg.package}/bin/craked";
        Restart = "on-failure";
        RuntimeDirectory = "crake";
        RuntimeDirectoryPreserve = "restart";
        StateDirectory = "crake";
        AmbientCapabilities = [ "CAP_NET_ADMIN" "CAP_NET_RAW" "CAP_NET_BIND_SERVICE" ];
        CapabilityBoundingSet = [ "CAP_NET_ADMIN" "CAP_NET_RAW" "CAP_NET_BIND_SERVICE" "CAP_KILL" ];
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        KillMode = "mixed";
      };
    };
  };
}
