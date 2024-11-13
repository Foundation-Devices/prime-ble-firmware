# SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
# SPDX-License-Identifier: GPL-3.0-or-later

# Build production firmware package
build:
    cargo xtask build-fw-image

# Build debug firmware package with UART console without flash protection
build-debug:
    cargo xtask build-fw-debug-image

# Flash SoftDevice and run Bluetooth test app with UART MPU
bluetooth-app:
    probe-rs erase --chip nrf52805_xxAA --allow-erase-all && probe-rs download ./misc/s112_nrf52_7.2.0_softdevice.hex --chip nrf52805_xxAA --binary-format hex && cd firmware && cargo bluetooth-app

# Flash and run debug version of main application without Cobs encoding and console UART
bluetooth-app-debug:
    cd firmware && cargo deb

# Flash and run debug version of bootloader without flash protection and unsigned firmware
bootloader-debug:
    cd bootloader && cargo deb

# Run COBS protocol size validation tests
cobs-size-test:
    cd host-protocol && cargo test -- --nocapture