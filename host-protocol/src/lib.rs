// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! MPU to BLE MCU communication protocol.
//! The MPU running keyOS is the host and nRF52x BLE is the target MCU.
//! WIll move here simpler struct for postcard messages!!

#![no_std]
use serde::{Deserialize, Serialize};

/// Maximum supported message size to be serialized or deserialized by `postcard`.
pub const COBS_MAX_MSG_SIZE: usize = 512;

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
    AckEraseFirmware,
    VerifyFirmware,
    AckVerifyFirmware { result: bool, hash: [u8; 32] },
    NackWithIdx { block_idx: usize },
    AckWithIdx { block_idx: usize },
    AckWithIdxCrc { block_idx: usize, crc: u32 },
    WriteFirmwareBlock { block_idx: usize, block_data: &'a [u8] },
    FirmwareOutOfBounds { block_idx: usize },
    NoCosignHeader,
    FirmwareVersion,
    AckFirmwareVersion { version: &'a str },
}

/// Host protocol messages.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum HostProtocolMessage<'a> {
    Bluetooth(#[serde(borrow)] Bluetooth<'a>),
    Bootloader(#[serde(borrow)] Bootloader<'a>),
    Reset,
}
