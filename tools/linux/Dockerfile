# Linux build and test environment for Humcase (Tauri 2). Ubuntu 22.04 on purpose: the AppImage
# links against this system's glibc, so it runs on any distribution at least as recent.
# Rust and Node caches live in Docker volumes (see build.sh), not in the image.
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/cache/rustup \
    CARGO_HOME=/cache/cargo \
    PATH=/cache/cargo/bin:$PATH \
    NO_STRIP=true \
    APPIMAGE_EXTRACT_AND_RUN=1

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential curl wget file git ca-certificates pkg-config xz-utils patchelf \
        libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
        libayatana-appindicator3-dev librsvg2-dev libssl-dev libxdo-dev \
        squashfs-tools desktop-file-utils xdg-utils \
        xvfb openbox x11-apps imagemagick xdotool dbus-x11 fonts-dejavu-core \
        libfuse2 binutils \
    && rm -rf /var/lib/apt/lists/*

# Node.js LTS.
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

# rustup only; the toolchain pinned by src-tauri/rust-toolchain.toml is installed on first build
# into the /cache volume.
RUN mkdir -p /cache && curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none \
    && chmod -R a+rwX /cache

WORKDIR /build
