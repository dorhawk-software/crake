# A bare Ubuntu, Debian or Fedora (IMAGE, e.g. ubuntu:22.04, debian:12,
# fedora:44) that boots systemd, for scripts/deb-test.sh: only what booting
# needs (systemd, D-Bus); apt or dnf pulls in the rest of crake's
# dependencies. Docker's Ubuntu images keep packages from starting services
# (policy-rc.d); a real system doesn't, so neither does this one.
ARG IMAGE=ubuntu:22.04
FROM ${IMAGE}
RUN if command -v apt-get >/dev/null; then \
      apt-get update \
      && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends systemd systemd-sysv dbus \
      && rm -rf /var/lib/apt/lists/* && rm -f /usr/sbin/policy-rc.d; \
    else \
      dnf install -y -q systemd dbus-broker util-linux dbus-tools && dnf clean all; \
    fi
STOPSIGNAL SIGRTMIN+3
CMD ["/sbin/init"]
