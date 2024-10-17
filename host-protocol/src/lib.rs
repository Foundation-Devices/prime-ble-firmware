// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! MPU to BLE MCU communication protocol.
//! The MPU running keyOS is the host and nRF52x BLE is the target MCU.
//! WIll move here simpler struct for postcard messages!!

#![no_std]
use serde::{Deserialize, Serialize};

/// Maximum supported message size to be serialized or deserialized by `postcard`.
pub const COBS_MAX_MSG_SIZE: usize = 512;

/// Number of UICR registers for secret value
pub const SECRET_UICR_SIZE: u16 = 4;

/// Bluetooth-specific messages.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum Bluetooth<'a> {
    Enable,
    Disable,

    GetSignalStrength,
    SignalStrength(u8),

    SendData(&'a [u8]),
    ReceivedData(&'a [u8]),
    GetFirmwareVersion,
    AckFirmwareVersion { version: &'a str },
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
    BootloaderVersion,
    AckBootloaderVersion { version: &'a str },
    // Challenge cmds
    ChallengeSet { secret: [u32; 4] }, // better to use an array u8; 32]?
    AckChallengeSet { result: SecretSaveResponse },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub enum SecretSaveResponse {
    /// Secret have been saved previously - no more allowed to write it again
    NotAllowed,
    /// Secret has been correctly saved
    Sealed,
    /// Error during saving secret
    Error,
}

/// BLE controller state.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum State {
    /// BLE is enabled and ready to send/receive data.
    Enabled,
    /// BLE is disabled. No wireless data transfer is possible.
    Disabled,
    /// BLE is in the process of upgrading its firmware (bootloader mode).
    FirmwareUpgrade,
    /// BLE is in an unknown, transient state.
    Unknown,
}

/// Host protocol messages.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum HostProtocolMessage<'a> {
    Bluetooth(#[serde(borrow)] Bluetooth<'a>),
    Bootloader(#[serde(borrow)] Bootloader<'a>),
    Reset,
    GetState,
    AckState(State),
    ChallengeRequest { challenge: u128, nonce: u32 },
    ChallengeResult { result: [u8; 32] },
}
