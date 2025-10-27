// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

//! MPU to BLE MCU communication protocol.
//! The MPU running keyOS is the host and nRF52x BLE is the target MCU.
//! Defines message types and structures for communication between the two processors.

#![no_std]

use bitflags::bitflags;
use consts::APP_MTU;
use heapless::Vec;
use serde::{Deserialize, Serialize};

/// Maximum supported message size to be serialized or deserialized by `postcard`.
/// Messages larger than this will be rejected.
pub const MAX_MSG_SIZE: usize = 270;

bitflags! {
    #[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
    pub struct AdvChan: u8 {
        const C39 = 1 << 7;
        const C38 = 1 << 6;
        const C37 = 1 << 5;
    }
}

pub type Message = Vec<u8, APP_MTU>;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum TxPower {
    Negative40dBm,
    Negative20dBm,
    Negative16dBm,
    Negative12dBm,
    Negative8dBm,
    Negative4dBm,
    ZerodBm,
    Positive3dBm,
    Positive4dBm,
}

impl From<TxPower> for i8 {
    fn from(value: TxPower) -> Self {
        match value {
            TxPower::Negative40dBm => -40,
            TxPower::Negative20dBm => -20,
            TxPower::Negative16dBm => -16,
            TxPower::Negative12dBm => -12,
            TxPower::Negative8dBm => -8,
            TxPower::Negative4dBm => -4,
            TxPower::ZerodBm => 0,
            TxPower::Positive3dBm => 3,
            TxPower::Positive4dBm => 4,
        }
    }
}

/// Bluetooth-specific messages for controlling the BLE radio and data transfer.
///
/// Make sure to only append new messages at the end of the enum, to keep backward compatibility
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum Bluetooth<'a> {
    /// Disable some Adv Channels
    DisableChannels(AdvChan),
    /// Acknowledge Adv Channels disabled
    AckDisableChannels,
    /// Negative acknowledgment of Adv Channels disabling
    NackDisableChannels,
    /// Turn on the BLE radio
    Enable,
    /// BLE radio enabled
    AckEnable,
    /// Turn off the BLE radio
    Disable,
    /// BLE radio disabled
    AckDisable,

    /// Request current signal strength
    GetSignalStrength,
    /// Response with signal strength value
    SignalStrength(Option<i8>),

    /// Send raw data over BLE connection
    SendData(Message),
    /// Response to data send request
    SendDataResponse(SendDataResponse),

    /// Request latest received data (if any)
    GetReceivedData,
    /// Data received over BLE connection
    ReceivedData(Message),
    /// No data has been received since last `GetReceivedData` request
    NoReceivedData,

    /// Request BLE firmware version
    GetFirmwareVersion,
    /// Response with firmware version string
    AckFirmwareVersion { version: &'a str },

    /// Get bt address
    GetBtAddress,
    /// Send bt address
    AckBtAddress { bt_address: [u8; 6] },

    /// Set Tx Output Power
    SetTxPower { power: TxPower },
    /// Tx Output Power set
    AckTxPower,

    /// Get device id
    GetDeviceId,
    /// Send device id
    AckDeviceId { device_id: [u8; 8] },
}

impl Bluetooth<'_> {
    pub fn is_request(&self) -> bool {
        match self {
            Self::DisableChannels(_) => true,
            Self::AckDisableChannels => false,
            Self::NackDisableChannels => false,
            Self::Enable => true,
            Self::AckEnable => false,
            Self::Disable => true,
            Self::AckDisable => false,
            Self::GetSignalStrength => true,
            Self::SignalStrength(_) => false,
            Self::SendData(_) => true,
            Self::SendDataResponse(_) => false,
            Self::GetReceivedData => true,
            Self::ReceivedData(_) => false,
            Self::NoReceivedData => false,
            Self::GetFirmwareVersion => true,
            Self::AckFirmwareVersion { .. } => false,
            Self::GetBtAddress => true,
            Self::AckBtAddress { .. } => false,
            Self::SetTxPower { .. } => true,
            Self::AckTxPower => false,
            Self::GetDeviceId => true,
            Self::AckDeviceId { .. } => false,
        }
    }
}

/// Bootloader-specific messages for firmware updates and verification.
///
/// Make sure to only append new messages at the end of the enum, to keep backward compatibility
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum Bootloader<'a> {
    /// Request to erase current firmware
    EraseFirmware,
    /// Acknowledge firmware erasure complete
    AckEraseFirmware,
    /// Negative acknowledgment of firmware erase, error during Read unaligned chunk
    NackEraseFirmwareRead,
    /// Negative acknowledgment of firmware erase, error during Erasing
    NackEraseFirmware,
    /// Negative acknowledgment of firmware erase, error during Write back unaligned chunk
    NackEraseFirmwareWrite,
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
    BootFirmware { trust: TrustLevel },
}

impl Bootloader<'_> {
    pub fn is_request(&self) -> bool {
        match self {
            Self::EraseFirmware => true,
            Self::AckEraseFirmware => false,
            Self::NackEraseFirmwareRead => false,
            Self::NackEraseFirmware => false,
            Self::NackEraseFirmwareWrite => false,
            Self::AckVerifyFirmware { .. } => false,
            Self::NackWithIdx { .. } => false,
            Self::AckWithIdx { .. } => false,
            Self::AckWithIdxCrc { .. } => false,
            Self::WriteFirmwareBlock { .. } => true,
            Self::FirmwareOutOfBounds { .. } => false,
            Self::NoCosignHeader => false,
            Self::FirmwareVersion => true,
            Self::AckFirmwareVersion { .. } => false,
            Self::BootloaderVersion => true,
            Self::AckBootloaderVersion { .. } => false,
            Self::ChallengeSet { .. } => true,
            Self::AckChallengeSet { .. } => false,
            Self::BootFirmware { .. } => true,
        }
    }
}

/// Response codes for challenge secret saving operations
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SecretSaveResponse {
    /// Secret has already been saved - cannot be overwritten
    NotAllowed,
    /// Secret was successfully saved and sealed
    Sealed,
    /// Error occurred while saving secret
    Error,
}

/// Firmware trust levels
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrustLevel {
    /// Full trust required
    Full,
    /// Development firmwares allowed
    Developer,
}

/// Current operational state of the BLE controller
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
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

/// Errors that can occur during postcard serialization or deserialization
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostcardError {
    /// Error deserializing message
    Deser,
    /// Buffer overflow
    OverFull,
}

/// Response codes for sending data over BLE connection
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SendDataResponse {
    /// Data sent successfully
    Sent,

    /// Data was not sent due to buffer being full
    BufferFull,
}

/// Top-level message types for host-target communication
///
/// Make sure to only append new messages at the end of the enum, to keep backward compatibility
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
    /// Postcard error
    PostcardError(PostcardError),

    /// An inappropriate message was received for the current state
    InappropriateMessage(State),
}

impl HostProtocolMessage<'_> {
    pub fn is_request(&self) -> bool {
        match self {
            Self::Bluetooth(sub) => sub.is_request(),
            Self::Bootloader(sub) => sub.is_request(),
            Self::Reset => true,
            Self::GetState => true,
            Self::AckState(_) => false,
            Self::ChallengeRequest { .. } => true,
            Self::ChallengeResult { .. } => false,
            Self::PostcardError(_) => false,
            Self::InappropriateMessage(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use postcard::to_slice;
    use std::println;
    use std::vec::Vec;

    fn encoded_data(msg: HostProtocolMessage) -> Vec<u8> {
        let mut buf = [0u8; 512]; // Buffer large enough for all messages
        let serialized = to_slice(&msg, &mut buf).unwrap();
        serialized.to_vec()
    }

    fn check_messages(name: &str, messages: &[(HostProtocolMessage, &[u8])]) {
        println!("----------------------------------");
        println!("{name} messages data after encoding:");
        println!("----------------------------------");
        for (msg, wanted) in messages {
            println!("{msg:02x?}");
            // make sure these encoded messages stay the same for backward compatibility
            let encoded = encoded_data(msg.clone());
            assert!(encoded.starts_with(wanted), "{encoded:02x?}\ndid not start with\n{wanted:02x?}");
        }
    }

    #[test]
    fn check_base_messages() {
        // Test each variant
        check_messages(
            "Base",
            &[
                (HostProtocolMessage::Reset, &[2]),
                (HostProtocolMessage::GetState, &[3]),
                (HostProtocolMessage::AckState(State::Disabled), &[4, 1]),
                (HostProtocolMessage::AckState(State::Enabled), &[4, 0]),
                (HostProtocolMessage::AckState(State::FirmwareUpgrade), &[4, 2]),
                (HostProtocolMessage::AckState(State::Unknown), &[4, 3]),
                (HostProtocolMessage::ChallengeRequest { nonce: 0 }, &[5, 0]),
                (
                    HostProtocolMessage::ChallengeResult { result: [0u8; 32] },
                    &[
                        6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    ],
                ),
                (HostProtocolMessage::PostcardError(PostcardError::Deser), &[7, 0]),
                (HostProtocolMessage::PostcardError(PostcardError::OverFull), &[7, 1]),
                (HostProtocolMessage::InappropriateMessage(State::Disabled), &[8, 1]),
                (HostProtocolMessage::InappropriateMessage(State::Enabled), &[8, 0]),
                (HostProtocolMessage::InappropriateMessage(State::FirmwareUpgrade), &[8, 2]),
                (HostProtocolMessage::InappropriateMessage(State::Unknown), &[8, 3]),
            ],
        );
    }

    #[test]
    fn check_bootloader_messages() {
        // Test each variant
        check_messages(
            "Bootloader",
            &[
                (HostProtocolMessage::Bootloader(Bootloader::EraseFirmware), &[1, 0]),
                (HostProtocolMessage::Bootloader(Bootloader::AckEraseFirmware), &[1, 1]),
                (HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmwareRead), &[1, 2]),
                (HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmware), &[1, 3]),
                (HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmwareWrite), &[1, 4]),
                (
                    HostProtocolMessage::Bootloader(Bootloader::AckVerifyFirmware {
                        result: true,
                        hash: [0; 32],
                    }),
                    &[
                        1, 5, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    ],
                ),
                (
                    HostProtocolMessage::Bootloader(Bootloader::NackWithIdx { block_idx: 0xFFFFFFFF }),
                    &[1, 6, 255, 255, 255, 255, 15],
                ),
                (
                    HostProtocolMessage::Bootloader(Bootloader::AckWithIdx { block_idx: 0xFFFFFFFF }),
                    &[1, 7, 255, 255, 255, 255, 15],
                ),
                (
                    HostProtocolMessage::Bootloader(Bootloader::AckWithIdxCrc {
                        block_idx: 0xFFFFFFFF,
                        crc: 0xFFFFFFFF,
                    }),
                    &[1, 8, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15],
                ),
                (
                    HostProtocolMessage::Bootloader(Bootloader::WriteFirmwareBlock {
                        block_idx: 0xFFFFFFFF,
                        block_data: &[0xFF; 256],
                    }),
                    &[
                        1, 9, 255, 255, 255, 255, 15, 128, 2, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    ],
                ),
                (
                    HostProtocolMessage::Bootloader(Bootloader::FirmwareOutOfBounds { block_idx: 0xFFFFFFFF }),
                    &[1, 10, 255, 255, 255, 255, 15],
                ),
                (HostProtocolMessage::Bootloader(Bootloader::NoCosignHeader), &[1, 11]),
                (HostProtocolMessage::Bootloader(Bootloader::FirmwareVersion), &[1, 12]),
                (
                    HostProtocolMessage::Bootloader(Bootloader::AckFirmwareVersion { version: "v1.2.3" }),
                    &[1, 13, 6, 118, 49, 46, 50, 46, 51],
                ),
                (HostProtocolMessage::Bootloader(Bootloader::BootloaderVersion), &[1, 14]),
                (
                    HostProtocolMessage::Bootloader(Bootloader::AckBootloaderVersion {
                        version: "v1.2.3-longversionstring",
                    }),
                    &[
                        1, 15, 24, 118, 49, 46, 50, 46, 51, 45, 108, 111, 110, 103, 118, 101, 114, 115, 105, 111, 110, 115, 116, 114, 105,
                        110, 103,
                    ],
                ),
                (
                    HostProtocolMessage::Bootloader(Bootloader::ChallengeSet { secret: [0xFFFFFFFF; 8] }),
                    &[
                        1, 16, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255,
                        255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15,
                    ],
                ),
                (
                    HostProtocolMessage::Bootloader(Bootloader::AckChallengeSet {
                        result: SecretSaveResponse::Error,
                    }),
                    &[1, 17, 2],
                ),
                // IMPORTANT: These need to start with [1, 18] to be compatible with pre-3.0 bootloaders.
                (
                    HostProtocolMessage::Bootloader(Bootloader::BootFirmware { trust: TrustLevel::Full }),
                    &[1, 18, 0],
                ),
                (
                    HostProtocolMessage::Bootloader(Bootloader::BootFirmware {
                        trust: TrustLevel::Developer,
                    }),
                    &[1, 18, 1],
                ),
            ],
        );
    }

    #[test]
    fn check_bluetooth_messages() {
        check_messages(
            "Bluetooth",
            &[
                (
                    HostProtocolMessage::Bluetooth(Bluetooth::DisableChannels(AdvChan::C37 | AdvChan::C38)),
                    &[0, 0, 96],
                ),
                (
                    HostProtocolMessage::Bluetooth(Bluetooth::DisableChannels(AdvChan::C37 | AdvChan::C39)),
                    &[0, 0, 160],
                ),
                (
                    HostProtocolMessage::Bluetooth(Bluetooth::DisableChannels(AdvChan::C38 | AdvChan::C39)),
                    &[0, 0, 192],
                ),
                (HostProtocolMessage::Bluetooth(Bluetooth::AckDisableChannels), &[0, 1]),
                (HostProtocolMessage::Bluetooth(Bluetooth::NackDisableChannels), &[0, 2]),
                (HostProtocolMessage::Bluetooth(Bluetooth::Enable), &[0, 3]),
                (HostProtocolMessage::Bluetooth(Bluetooth::AckEnable), &[0, 4]),
                (HostProtocolMessage::Bluetooth(Bluetooth::Disable), &[0, 5]),
                (HostProtocolMessage::Bluetooth(Bluetooth::AckDisable), &[0, 6]),
                (HostProtocolMessage::Bluetooth(Bluetooth::GetSignalStrength), &[0, 7]),
                (
                    HostProtocolMessage::Bluetooth(Bluetooth::SignalStrength(Some(i8::MAX))),
                    &[0, 8, 1, 127],
                ),
                (
                    HostProtocolMessage::Bluetooth(Bluetooth::SendData(heapless::Vec::from_iter([0xFF; APP_MTU].into_iter()))),
                    &[
                        0, 9, 244, 1, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    ],
                ),
                (
                    HostProtocolMessage::Bluetooth(Bluetooth::SendDataResponse(SendDataResponse::Sent)),
                    &[0, 10, 0],
                ),
                (HostProtocolMessage::Bluetooth(Bluetooth::GetReceivedData), &[0, 11]),
                (
                    HostProtocolMessage::Bluetooth(Bluetooth::ReceivedData(heapless::Vec::from_iter([0xFF; APP_MTU].into_iter()))),
                    &[
                        0, 12, 244, 1, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    ],
                ),
                (HostProtocolMessage::Bluetooth(Bluetooth::NoReceivedData), &[0, 13]),
                (HostProtocolMessage::Bluetooth(Bluetooth::GetFirmwareVersion), &[0, 14]),
                (
                    HostProtocolMessage::Bluetooth(Bluetooth::AckFirmwareVersion { version: "v1.2.3" }),
                    &[0, 15, 6, 118, 49, 46, 50, 46, 51],
                ),
                (HostProtocolMessage::Bluetooth(Bluetooth::GetBtAddress), &[0, 16]),
                (
                    HostProtocolMessage::Bluetooth(Bluetooth::AckBtAddress { bt_address: [0xFF; 6] }),
                    &[0, 17, 255, 255, 255, 255, 255, 255],
                ),
                (
                    HostProtocolMessage::Bluetooth(Bluetooth::SetTxPower {
                        power: TxPower::Positive4dBm,
                    }),
                    &[0, 18, 8],
                ),
                (HostProtocolMessage::Bluetooth(Bluetooth::AckTxPower), &[0, 19]),
                (HostProtocolMessage::Bluetooth(Bluetooth::GetDeviceId), &[0, 20]),
                (
                    HostProtocolMessage::Bluetooth(Bluetooth::AckDeviceId { device_id: [0xFF; 8] }),
                    &[0, 21, 255, 255, 255, 255, 255, 255, 255, 255],
                ),
            ],
        );
    }
}
