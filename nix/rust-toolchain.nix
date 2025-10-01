# SPDX-FileCopyrightText: 2025 Foundation Devices, Inc. <hello@foundation.xyz>
# SPDX-License-Identifier: GPL-3.0-or-later
{
  self,
  system,
  pkgs,
  fenix,
}:
let
  toolchainSha256 = "sha256-18J/HvJzns0BwRNsUNkCSoIw7MtAmppPu5jNzuzMvCc=";

  baseToolchain = fenix.packages.${system}.fromToolchainFile {
    file = self + "/rust-toolchain.toml";
    sha256 = toolchainSha256;
  };

  thumbv7emStd = fenix.packages.${system}.targets.thumbv7em-none-eabi.fromToolchainFile {
    file = self + "/rust-toolchain.toml";
    sha256 = toolchainSha256;
  };

in
{
  rust-ble-firmware = fenix.packages.${system}.combine [
    baseToolchain
    thumbv7emStd
  ];
  rust-analyzer = fenix.packages.${system}.rust-analyzer;
}
