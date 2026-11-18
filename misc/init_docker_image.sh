#!/usr/bin/bash
set -exuo pipefail

export TZ=America/New_York
export DEBIAN_FRONTEND=noninteractive
PATH="/root/.cargo/bin:${PATH}"

apt update
apt upgrade -y --no-install-recommends
apt install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    curl \
    gcc-arm-none-eabi \
    gh \
    git \
    just \
    libclang-dev \
    openssh-client \
    pkg-config \
    sudo \

# Make the layer a bit smaller
apt clean
rm -rf /var/lib/apt/lists/*

# Allow raw uid setting for sudo in docker_run_wrapper.sh
echo "Defaults runas_allow_unknown_id" >>/etc/sudoers

# Log in to github. Note: this will store credentials in the image in /root
gh auth login --with-token < .github-access-token
gh auth setup-git

curl --proto '=https' --tlsv1.3 https://sh.rustup.rs -sSf | sh -s -- -y

# Also installs the required components in rust-toolchain.toml
rustup show

cargo install cargo-binutils

# Install cosign2 v3.0.0 from the same commit used by Cargo and Nix.
COSIGN2_REV="45c233c1a0c08f06c5cafc36b754ba6323c5ed86"
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo install --locked \
    --git https://github.com/Foundation-Devices/cosign2.git --rev "${COSIGN2_REV}" cosign2-bin

# Give r/w access and execute on directories to everyone so that we can run builds
# with different UIDs while using root's cargo cache
chmod -R ag+rwX /root
