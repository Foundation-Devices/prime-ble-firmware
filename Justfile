# SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
# SPDX-License-Identifier: GPL-3.0-or-later

build:
    cargo xtask build-fw-image

build-debug:
    cargo xtask build-fw-debug-image

flash:
    cd firmware && cargo run --release --bin firmware