# An X server with no display and a software GPU (lavapipe), for a real GPUI
# window (scripts/x11-test.sh). The test binary comes from the host, so the
# release matches the host's (its glibc).
ARG UBUNTU=26.04
FROM ubuntu:${UBUNTU}
RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      xvfb libxkbcommon-x11-0 libvulkan1 mesa-vulkan-drivers libfontconfig1 fonts-dejavu-core \
      adwaita-icon-theme \
 && rm -rf /var/lib/apt/lists/*
