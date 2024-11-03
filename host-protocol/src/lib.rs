// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! MPU to BLE MCU communication protocol.
//! The MPU running keyOS is the host and nRF52x BLE is the target MCU.
//! Defines message types and structures for communication between the two processors.

#![no_std]
use serde::{Deserialize, Serialize};

/// Maximum supported message size to be serialized or deserialized by `postcard`.
/// Messages larger than this will be rejected.
pub const COBS_MAX_MSG_SIZE: usize = 512;

/// Bluetooth-specific messages for controlling the BLE radio and data transfer.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum Bluetooth<'a> {
    /// Turn on the BLE radio
    Enable,
    /// Turn off the BLE radio
    Disable,

    /// Request current signal strength
    GetSignalStrength,
    /// Response with signal strength value (0-255)
    SignalStrength(u8),

    /// Send raw data over BLE connection
    SendData(&'a [u8]),
    /// Data received over BLE connection
    ReceivedData(&'a [u8]),
    /// Request BLE firmware version
    GetFirmwareVersion,
    /// Response with firmware version string
    AckFirmwareVersion { version: &'a str },
}

/// Bootloader-specific messages for firmware updates and verification.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum Bootloader<'a> {
    /// Request to erase current firmware
    EraseFirmware,
    /// Acknowledge firmware erasure complete
    AckEraseFirmware,
    /// Request firmware verification
    VerifyFirmware,
    /// Result of firmware verification with hash
    AckVerifyFirmware { result: bool, hash: [u8; 32] },
    /// Negative acknowledgment of block with index
    NackWithIdx { block_idx: usize },
    /// Positive acknowledgment of block with index
    AckWithIdx { block_idx: usize },
    /// Acknowledgment with block index and CRC
    AckWithIdxCrc { block_idx: usize, crc: u32 },
    /// Write firmware block at specified index
    WriteFirmwareBlock { block_idx: usize, block_data: &'a [u8] },
    /// Error: firmware block index out of valid range
    FirmwareOutOfBounds { block_idx: usize },
    /// Error: missing or invalid Cosign header
    NoCosignHeader,
    /// Request current firmware version
    FirmwareVersion,
    /// Response with firmware version string
    AckFirmwareVersion { version: &'a str },
    /// Request bootloader version
    BootloaderVersion,
    /// Response with bootloader version string
    AckBootloaderVersion { version: &'a str },
    /// Set challenge secret for authentication
    ChallengeSet { secret: [u32; 8] },
    /// Response to challenge secret setting
    AckChallengeSet { result: SecretSaveResponse },
    /// Boot firmware
    BootFirmware,
}

/// Response codes for challenge secret saving operations
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub enum SecretSaveResponse {
    /// Secret has already been saved - cannot be overwritten
    NotAllowed,
    /// Secret was successfully saved and sealed
    Sealed,
    /// Error occurred while saving secret
    Error,
}

/// Current operational state of the BLE controller
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum State {
    /// BLE radio is on and ready for communication
    Enabled,
    /// BLE radio is off
    Disabled,
    /// Device is in bootloader mode for firmware updates
    FirmwareUpgrade,
    /// Device state is undefined or transitioning
    Unknown,
}

/// Top-level message types for host-target communication
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum HostProtocolMessage<'a> {
    /// Bluetooth control and data transfer messages
    Bluetooth(#[serde(borrow)] Bluetooth<'a>),
    /// Bootloader and firmware update messages
    Bootloader(#[serde(borrow)] Bootloader<'a>),
    /// Request device reset
    Reset,
    /// Query current device state
    GetState,
    /// Response with current state
    AckState(State),
    /// Challenge request with nonce for authentication
    ChallengeRequest { nonce: u64 },
    /// Challenge response with authentication result
    ChallengeResult { result: [u8; 32] },
}
