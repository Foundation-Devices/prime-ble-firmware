# SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
# SPDX-License-Identifier: GPL-3.0-or-later

build:
    cargo xtask build-fw-image

build-debug:
    cargo xtask build-fw-debug-image

flash-debug-app:
    cd firmware && cargo deb

flash-debug-bootloader:
    cd bootloader && cargo deb

cobs-size-test:
    cd host-protocol && cargo test -- --nocapture