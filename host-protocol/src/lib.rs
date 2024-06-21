// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! MPU to BLE MCU communication protocol.
//! The MPU running keyOS is the host and nRF52x BLE is the target MCU.
//! WIll move here simpler struct for postcard messages!!

#![no_std]
use num_enum::TryFromPrimitive;
use serde::{Deserialize, Serialize};

/// Bluetooth-specific messages.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum Bluetooth<'a> {
    Enable,
    Disable,

    GetSignalStrength,
    SignalStrength(u8),

    SendData(&'a [u8]),
    ReceivedData(&'a [u8]),
}

/// Bootloader-specific messages.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum Bootloader<'a> {
    EraseFirmware,
    WriteFirmwareBlock {
        block_idx: usize,
        block_data: &'a [u8],
    },
}

/// Host protocol messages.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum HostProtocolMessage<'a> {
    Bluetooth(#[serde(borrow)] Bluetooth<'a>),
    Bootloader(#[serde(borrow)] Bootloader<'a>),
    Reset,
}

// TODO: status       - read status info (status maks, num NUS packets received, num. connections, etc.)
// TODO: ble_nus_send - send a packet via BLE NUS (Nordic UART emulation over BLE)
// TODO: ble_nus_recv - read last received packet from BLE NUS
