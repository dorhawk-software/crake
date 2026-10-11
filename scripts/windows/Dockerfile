# Checking Crake for Windows on Linux (scripts/windows-check.sh): Rust's
# GNU Windows target and MinGW, whose windres GPUI's manifest needs.
FROM rust:1-bookworm
RUN apt-get update \
 && apt-get install -y --no-install-recommends mingw-w64 \
 && rm -rf /var/lib/apt/lists/* \
 && rustup target add x86_64-pc-windows-gnu \
 && rustup component add clippy
