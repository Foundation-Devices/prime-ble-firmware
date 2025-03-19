# SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
# SPDX-License-Identifier: GPL-3.0-or-later

# Install requirments
install:
    cargo install cargo-binutils
    cargo install nrf-recover
    rustup component add llvm-tools
    rustup target add thumbv7em-none-eabi
    CC="" cargo install --path ../keyOS/imports/cosign2/cosign2-bin --bin cosign2

# Build production firmware package
build:
    cargo xtask build-fw-image

# Build debug firmware package with UART console without flash protection
build-debug:
    cargo xtask build-fw-debug-image

unlock:
    nrf-recover -y

flash:
    probe-rs download ./BtPackage/BTApp_Full_Image.hex --chip nrf52805_xxAA --binary-format hex --allow-erase-all

# Flash SoftDevice
softdevice:
    cargo xtask patch-sd
    probe-rs download ./misc/s113_nrf52_7.3.0_softdevice_patched.hex --chip nrf52805_xxAA --binary-format hex --allow-erase-all

# Flash and run Bluetooth test app with UART MPU
bluetooth-app: softdevice
    cd firmware && cargo run --release --features analytics

# Flash and run debug version of main application without Cobs encoding and console UART
bluetooth-app-debug: softdevice
    cd firmware && cargo run --release --features debug

# Flash and run debug version of bootloader without flash protection and unsigned firmware
bootloader-debug: softdevice
    cd bootloader && cargo run --release --features debug

# Run protocol encoding tests
test-encoding:
    cd host-protocol && cargo test -- --nocapture

# Send Host Protocol Enable Bluetooth command
enable-ble:
    cargo run --example host_control -- -c enable

# Send Host Protocol Disable Bluetooth command
disable-ble:
    cargo run --example host_control -- -c disable

# Send Host Protocol Get Firmware Version command
firmware-version:
    cargo run --example host_control -- -c fw-version

# Send Host Protocol Get Signal Strength command
rssi:
    cargo run --example host_control -- -c rssi

# Send Host Protocol Get BT Address command
bt-address:
    cargo run --example host_control -- -c address

# Update application firmware
update-app:
    cargo run --example host_control -- -c update-app

# Send data by BLE to "Passport Prime" peripheral
ble-send:
    cargo run -p host-ble -- -w
