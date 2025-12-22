<!--
SPDX-FileCopyrightText: 2025 Foundation Devices, Inc. <hello@foundation.xyz>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Reproducibility Guide

The instructions below describe how to easily build and verify Prime BLE firmware in a reproducible way.

Please note that this guide has been designed for Linux, so if you're running a different operating system the exact steps here may differ slightly. However, we've done our best to make them as portable as possible for other popular operating systems.

## What to Expect

In this guide we will outline the exact steps necessary to get set up, build firmware directly from the source code, and verify that it properly matches the published build hash and release binaries for any given version of Prime BLE's firmware. Once you've completed the steps outlined here, you'll have verified fully that the source code for a given version does indeed match the binaries we release to you. This ensures that nothing outside of the open-source code has been included in any given release, and that the released binaries are indeed built directly from the publicly available source code.

Security through transparency is the goal here, and firmware reproducibility is a key aspect of that!

## Setup

In order to build and verify the reproducibility of Prime BLE firmware, you will need to:

- Get the source code and checkout the correct tag
- Install Nix package manager with flakes enabled
- Set up the development environment using the flake
- Build the reproducible binaries
- Verify the binaries match the published build hash

We'll walk through every step above in this guide to ensure you can build and verify any version of Prime BLE's firmware easily.

### **Get the Source Code**

The instructions below assume you are installing into your home folder at `~/prime-ble-firmware`. You can choose to install to a different folder, and just update command paths appropriately.

**Important**: Before building, you must checkout the specific git tag you want to reproduce. This ensures you're building the exact same code as the published release.

```bash
cd ~/
git clone https://github.com/Foundation-Devices/prime-ble-firmware.git
cd prime-ble-firmware
git checkout app_v3.2.0  # Replace with the tag you want to reproduce
```

### **Install Nix Package Manager**

Nix is a powerful package manager that provides reproducible builds and development environments. The reproducibility of Prime BLE firmware is guaranteed through the use of our Nix flake, which pins exact versions of all tools and dependencies.

#### Install Nix

Follow the official Nix installation guide for your operating system:

- [Official Nix Installation Guide](https://nixos.org/download.html)

For Linux and macOS, the recommended installer is:

```bash
curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
```

#### Enable Flakes

After installing Nix, you need to enable experimental flakes features. Create or edit `~/.config/nix/nix.conf`:

```bash
mkdir -p ~/.config/nix
echo "experimental-features = nix-command flakes" >> ~/.config/nix/nix.conf
```

Or, you can enable flakes temporarily by setting an environment variable:

```bash
export NIX_CONFIG="experimental-features = nix-command flakes"
```

### **Set Up Development Environment**

Once Nix is installed with flakes enabled, use the flake in this repository to set up the reproducible development environment:

```bash
nix develop
```

This command:
- Downloads the exact toolchain versions specified in `flake.nix`
- Sets up the development environment with all required tools
- Ensures you're using the same toolchain as our official builds

The environment includes:
- Rust toolchain with the exact version from `rust-toolchain.toml`
- Build tools (cargo-binutils, gcc-arm-embedded, etc.)
- Signing tools (cosign2)
- Development utilities (just, git)

## **Building Prime BLE Firmware**

### Architecture and Toolchain Verification

Our build system includes automatic checks to ensure reproducibility:

1. **Architecture Check**: Verifies you're building on `x86_64` - this is required for official reproducible builds
2. **Toolchain Check**: Verifies you're using the Nix-provided toolchain from the nix store

These checks are built into the `build-unsigned` command and will warn you if you're not using the correct environment.

### Build the Firmware

Now that we have everything in place, we can build the firmware binaries for Prime BLE with a simple command:

```bash
just build-unsigned
```

This command:
1. Runs the architecture and toolchain verification checks
2. Builds the bootloader and firmware in release mode
3. Outputs the binaries to `BtPackage/` directory
4. Displays binary sizes and flash usage information
5. Shows the SHA256 hash of the built files for verification

The build process will take a few minutes as it compiles the bootloader and application from source. Once complete, you'll have:
- `BtPackage/bootloader.bin` - The compiled bootloader
- `BtPackage/BT_application.bin` - The compiled application

**Note**: This builds unsigned firmware. If you want to create a fully flashable package, you would need to sign the firmware using our cosign2 process, but for reproducibility verification, the unsigned binaries are sufficient.

## Verifying Prime BLE Firmware

### Verify Build Hash

To verify that the binary produced matches what you should see, you can calculate the SHA256 hash of the built files and compare it to the published hash in the GitHub release notes.

```bash
# Calculate SHA256 hash of the built binaries
sha256sum BtPackage/bootloader.bin BtPackage/BT_application.bin
```

Example output:
```
a1b2c3d4e5f6...  BtPackage/bootloader.bin
f6e5d4c3b2a1...  BtPackage/BT_application.bin
```

Compare these hashes with the ones published in the GitHub release notes for the tag you checked out. If the hashes match, congratulations! You've successfully verified that the firmware you built exactly matches the source code published on GitHub.

If your hashes do not match for any reason, stop immediately and contact us at [hello@foundation.xyz](mailto:hello@foundation.xyz)! We'll help you investigate the cause of this discrepancy and get to the bottom of the issue.

## Reproducibility Guarantee

The reproducibility of Prime BLE firmware is guaranteed by:

1. **Nix Flakes**: The `flake.nix` file pins exact versions of all dependencies, ensuring the same tools are used every time
2. **Architecture Verification**: Only `amd64` builds are considered official reproducible builds
3. **Toolchain Verification**: The build system checks that the toolchain comes from the Nix store
4. **Deterministic Builds**: The build process avoids timestamps and other non-deterministic factors

When you follow these steps exactly:
- Using the same git tag
- Using the Nix development environment  
- Building on amd64 architecture
- Using the provided build commands

You will get bit-for-bit identical binaries every time, and identical to our official builds.

## Installing Firmware

For installing firmware on your device, please use the official signed binaries from the GitHub releases. The unsigned binaries you built are for verification purposes only.

## Conclusion

We want to close out this guide by thanking our fantastic community. Open source and the verifiability and transparency it brings are core to our ethos at Foundation, and the ability to reproducibly build firmware for Prime BLE is a core outpouring of that.

We can't wait to see more of our community take this additional step and remove a little more trust from the process by verifying that each build is reproducible.
