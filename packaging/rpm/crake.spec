# crake's RPM (Fedora): the files scripts/build-deb.sh built and staged in
# target/deb/root, the binaries the .deb ships, packed for rpm.
# Built by scripts/build-rpm.sh, tested by scripts/deb-test.sh fedora:NN.
Name:           crake
Version:        %{crake_version}
Release:        1%{?dist}
Summary:        VPN client that routes only what needs the tunnel
License:        GPL-3.0-or-later
URL:            https://dorhawk.software/crake/
ExclusiveArch:  x86_64

# At least one engine; openvpn3 isn't in Fedora (its own repository).
# openvpn is Recommended, not an alternative here: dnf met "(openconnect or
# openvpn)" with openvpn, leaving AnyConnect users without openconnect.
Requires:       (openconnect or openvpn3)
Requires:       nftables
Requires:       iproute
Requires:       systemd-resolved
Requires:       polkit
Requires:       dbus
Recommends:     openvpn
Recommends:     gnome-shell-extension-appindicator
Recommends:     gnome-keyring
Suggests:       openvpn3
# IPsec (IKEv2) tunnels: craked's charon (its unit, crake-charon.service).
# Fedora's strongswan service is off until someone enables it.
Recommends:     strongswan

%description
Crake connects to AnyConnect-compatible gateways (and the other protocols
openconnect speaks), OpenVPN profiles and IPsec (IKEv2) gateways, routes
nothing by default, and learns per host what needs a tunnel: a host that
doesn't answer directly but answers through a tunnel is sent through just
that tunnel. Several tunnels can be up side by side. Everything it decides
happens on this computer; nothing is sent anywhere.

%install
cp -a %{crake_root}/. %{buildroot}/
# Fedora confines services with SELinux, not AppArmor.
rm -rf %{buildroot}/etc/apparmor.d
# Fedora's charon, where Debian's is in the staged unit.
sed -i 's#/usr/lib/ipsec/charon#/usr/libexec/strongswan/charon#' \
    %{buildroot}/usr/lib/systemd/system/crake-charon.service

%post
if [ -d /run/systemd/system ]; then
    systemctl daemon-reload >/dev/null 2>&1 || :
    systemctl reload dbus.service >/dev/null 2>&1 || :
    if [ "$1" -ge 2 ] && systemctl is-active --quiet craked.service; then
        # An upgrade: the running craked hands its tunnels to the new one,
        # signed in (craked's manager/handover.rs), instead of logging off.
        mkdir -p /run/crake
        : > /run/crake/handover
        systemctl restart craked.service >/dev/null 2>&1 || :
    else
        systemctl enable --now craked.service >/dev/null 2>&1 || :
    fi
fi

%preun
# Removing (not upgrading): craked switches every tunnel off as it stops.
if [ "$1" -eq 0 ] && [ -d /run/systemd/system ]; then
    systemctl disable --now craked.service >/dev/null 2>&1 || :
fi

%postun
if [ "$1" -eq 0 ]; then
    [ -d /run/systemd/system ] && systemctl daemon-reload >/dev/null 2>&1 || :
    # What craked learned and logged goes with it.
    rm -rf /var/lib/crake
fi

%files
/usr/bin/crake
/usr/sbin/craked
/usr/libexec/crake
/usr/lib/systemd/system/craked.service
/usr/lib/systemd/system/crake-charon.service
/usr/share/dbus-1/system.d/software.dorhawk.Crake1.conf
/usr/share/polkit-1/actions/software.dorhawk.crake.policy
/usr/share/applications/software.dorhawk.crake.desktop
/usr/share/icons/hicolor/*/apps/software.dorhawk.crake.*
%dir /etc/crake
%config(noreplace) /etc/crake/craked.toml
%dir /etc/crake/managed.d
%license /usr/share/doc/crake/copyright
%doc /usr/share/doc/crake/craked.toml.example
%license /usr/share/doc/crake/OFL-AtkinsonHyperlegibleMono.txt
%license /usr/share/doc/crake/OFL-AtkinsonHyperlegibleNext.txt
