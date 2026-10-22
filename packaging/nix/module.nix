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
    ipsec = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        IPsec (IKEv2) tunnels: strongSwan's charon, which craked starts as
        crake-charon.service for the first IPsec tunnel switched on (as the
        distributions' packages suggest strongSwan rather than require it).
      '';
    };
    extraConfig = lib.mkOption {
      type = lib.types.lines;
      default = "";
      description = "More of /etc/crake/craked.toml (see the package's craked.toml.example).";
    };
  };

  config = lib.mkIf cfg.enable {
    # The app signs in with openconnect as the user; with IPsec, it finds
    # strongSwan in the system's profile (New tunnel's IPsec card).
    environment.systemPackages = [ cfg.package pkgs.openconnect pkgs.openvpn ]
      ++ lib.optional cfg.ipsec pkgs.strongswan;
    # charon is in strongSwan's libexec, which the profile leaves out unless
    # asked (/lib, with its plugins, is in).
    environment.pathsToLink = lib.optional cfg.ipsec "/libexec";
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
    '' + lib.optionalString cfg.ipsec ''
      charon = "${pkgs.strongswan}/libexec/ipsec/charon"
    '' + cfg.extraConfig;

    # As packaging/crake-charon.service: a /run of its own (charon's pid
    # file's path is built in, so it never meets a system strongSwan's),
    # craked's folder bound back in for its settings and sockets.
    systemd.services.crake-charon = lib.mkIf cfg.ipsec {
      description = "Crake's IPsec engine (strongSwan's charon)";
      bindsTo = [ "craked.service" ];
      after = [ "craked.service" ];
      environment = {
        STRONGSWAN_CONF = "/run/crake/ipsec/strongswan.conf";
        LC_ALL = "C";
      };
      serviceConfig = {
        Type = "simple";
        ExecStart = "${pkgs.strongswan}/libexec/ipsec/charon";
        TemporaryFileSystem = "/run";
        BindPaths = [ "/run/crake" "-/run/crake/ipsec/run:/run/strongswan" ];
        CapabilityBoundingSet = [ "CAP_NET_ADMIN" "CAP_NET_RAW" "CAP_IPC_LOCK" ];
        ProtectHome = true;
        PrivateTmp = true;
      };
    };

    # As packaging/craked.service, with the tools craked runs on its path.
    systemd.services.craked = {
      description = "Crake daemon";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" "dbus.service" ];
      wants = [ "network-online.target" ];
      path = [ pkgs.nftables pkgs.iproute2 pkgs.openconnect pkgs.openvpn ];
      environment = lib.mkIf cfg.ipsec { CRAKE_CHARON_UNIT = "crake-charon.service"; };
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
        # The log and the hosts are the service's alone (craked closes it too).
        StateDirectoryMode = "0700";
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
