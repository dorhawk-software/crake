# A bare Ubuntu that boots systemd, for scripts/deb-test.sh: only what
# booting needs (systemd, D-Bus); apt pulls in the rest of crake's Depends.
# Docker's Ubuntu images keep packages from starting services
# (policy-rc.d); a real system doesn't, so neither does this one.
ARG RELEASE=22.04
FROM ubuntu:${RELEASE}
RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends systemd systemd-sysv dbus \
 && rm -rf /var/lib/apt/lists/* \
 && rm -f /usr/sbin/policy-rc.d
STOPSIGNAL SIGRTMIN+3
CMD ["/sbin/init"]
