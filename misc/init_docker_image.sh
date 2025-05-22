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

# Install cosign2
git clone --no-checkout --depth 1 https://github.com/Foundation-Devices/keyOS.git
(
    cd keyOS
    git sparse-checkout set imports/cosign2 
    git checkout
    export CARGO_NET_GIT_FETCH_WITH_CLI=true
    cargo install --path imports/cosign2/cosign2-bin --bin cosign2
)
rm -rf keyOS

# Give r/w access and execute on directories to everyone so that we can run builds
# with different UIDs while using root's cargo cache
chmod -R ag+rwX /root
