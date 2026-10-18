# Build/flash environment for OpenSK (Wasefire) on the Feitian OpenSK V2 dongle. Nothing is installed on the host.
FROM debian:trixie-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl git build-essential pkg-config libudev-dev libssl-dev \
      openssl python3 usbutils uuid-runtime python3-fido2 \
    && rm -rf /var/lib/apt/lists/*

ARG UID=1000
ARG GID=1000
RUN groupadd -g ${GID} dev && useradd -m -u ${UID} -g ${GID} dev
USER dev
ENV RUSTUP_HOME=/home/dev/.rustup CARGO_HOME=/home/dev/.cargo PATH=/home/dev/.cargo/bin:$PATH

# Toolchain pinned by rust-toolchain.toml in OpenSK and Wasefire.
ARG TOOLCHAIN=nightly-2026-06-03
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
      --profile minimal --default-toolchain ${TOOLCHAIN} --no-modify-path \
      -c clippy,llvm-tools,miri,rust-src,rustfmt \
      -t thumbv7em-none-eabi \
    && rustup toolchain install stable --profile minimal \
    && mkdir -p /home/dev/.cargo/registry /home/dev/.cargo/git

WORKDIR /work

# nrfutil (Nordic) for DFU packaging/flashing: the first install must be able to overwrite a SoftDevice (--sd-req=0).
RUN mkdir -p /home/dev/.local/bin \
    && curl -fsSLo /home/dev/.local/bin/nrfutil \
      https://files.nordicsemi.com/artifactory/swtools/external/nrfutil/executables/x86_64-unknown-linux-gnu/nrfutil \
    && chmod +x /home/dev/.local/bin/nrfutil \
    && /home/dev/.local/bin/nrfutil install device nrf5sdk-tools
ENV PATH=/home/dev/.local/bin:$PATH
