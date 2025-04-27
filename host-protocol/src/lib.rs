// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

//! MPU to BLE MCU communication protocol.
//! The MPU running keyOS is the host and nRF52x BLE is the target MCU.
//! Defines message types and structures for communication between the two processors.

#![no_std]
use bitflags::bitflags;
use serde::{Deserialize, Serialize};

/// Maximum supported message size to be serialized or deserialized by `postcard`.
/// Messages larger than this will be rejected.
pub const COBS_MAX_MSG_SIZE: usize = 512;

bitflags! {
    #[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
    pub struct AdvChan: u8 {
        const C39 = 1 << 7;
        const C38 = 1 << 6;
        const C37 = 1 << 5;
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
    SendData(&'a [u8]),
    /// Response to data send request
    SendDataResponse(SendDataResponse),

    /// Request latest received data (if any)
    GetReceivedData,
    /// Data received over BLE connection
    ReceivedData(&'a [u8]),
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
    BootFirmware,
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
            Self::BootFirmware => true,
        }
    }
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

/// Errors that can occur during postcard serialization or deserialization
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum PostcardError {
    /// Error deserializing message
    Deser,
    /// Buffer overflow
    OverFull,
}

/// Response codes for sending data over BLE connection
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub enum SendDataResponse {
    /// Data sent successfully
    Sent,

    /// Data was not sent due to buffer being full
    BufferFull,

    /// Data was not sent due to being bigger than maximum APP_MTU size
    DataTooLarge,
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
    use postcard::{to_slice, to_slice_cobs};
    use std::println;
    use std::{vec, vec::Vec};

    fn encoded_data(msg: HostProtocolMessage) -> Vec<u8> {
        let mut buf = [0u8; 512]; // Buffer large enough for all messages
        let serialized = to_slice(&msg, &mut buf).unwrap();
        serialized.to_vec()
    }

    fn encoded_data_cobs(msg: HostProtocolMessage) -> Vec<u8> {
        let mut buf = [0u8; 512]; // Buffer large enough for all messages
        let serialized = to_slice_cobs(&msg, &mut buf).unwrap();
        serialized.to_vec()
    }

    #[test]
    fn check_base_messages() {
        // Test each variant
        let data_base_messages = [
            (HostProtocolMessage::Reset, vec![2, 2, 0], vec![2]),
            (HostProtocolMessage::GetState, vec![2, 3, 0], vec![3]),
            (HostProtocolMessage::AckState(State::Disabled), vec![3, 4, 1, 0], vec![4, 1]),
            (HostProtocolMessage::AckState(State::Enabled), vec![2, 4, 1, 0], vec![4, 0]),
            (HostProtocolMessage::AckState(State::FirmwareUpgrade), vec![3, 4, 2, 0], vec![4, 2]),
            (HostProtocolMessage::AckState(State::Unknown), vec![3, 4, 3, 0], vec![4, 3]),
            (HostProtocolMessage::ChallengeRequest { nonce: 0 }, vec![2, 5, 1, 0], vec![5, 0]),
            (
                HostProtocolMessage::ChallengeResult { result: [0u8; 32] },
                vec![
                    2, 6, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0,
                ],
                vec![
                    6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ],
            ),
            (
                HostProtocolMessage::PostcardError(PostcardError::Deser),
                vec![2, 7, 1, 0],
                vec![7, 0],
            ),
            (
                HostProtocolMessage::PostcardError(PostcardError::OverFull),
                vec![3, 7, 1, 0],
                vec![7, 1],
            ),
            (
                HostProtocolMessage::InappropriateMessage(State::Disabled),
                vec![3, 8, 1, 0],
                vec![8, 1],
            ),
            (
                HostProtocolMessage::InappropriateMessage(State::Enabled),
                vec![2, 8, 1, 0],
                vec![8, 0],
            ),
            (
                HostProtocolMessage::InappropriateMessage(State::FirmwareUpgrade),
                vec![3, 8, 2, 0],
                vec![8, 2],
            ),
            (
                HostProtocolMessage::InappropriateMessage(State::Unknown),
                vec![3, 8, 3, 0],
                vec![8, 3],
            ),
        ];

        println!("----------------------------------");
        println!("Base messages data after encoding:");
        println!("----------------------------------");
        for (msg, wanted_cobs, wanted) in data_base_messages {
            println!("{:02x?}", msg);
            // make sure these encoded messages stay the same for backward compatibility
            assert_eq!(encoded_data_cobs(msg.clone()), wanted_cobs);
            // make sure these encoded messages stay the same for backward compatibility
            assert_eq!(encoded_data(msg), wanted);
        }
    }

    #[test]
    fn check_bootloader_messages() {
        // Test each variant
        let data_bootloader_messages = [
            (
                HostProtocolMessage::Bootloader(Bootloader::EraseFirmware),
                vec![2, 1, 1, 0],
                vec![1, 0],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::AckEraseFirmware),
                vec![3, 1, 1, 0],
                vec![1, 1],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmwareRead),
                vec![3, 1, 2, 0],
                vec![1, 2],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmware),
                vec![3, 1, 3, 0],
                vec![1, 3],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmwareWrite),
                vec![3, 1, 4, 0],
                vec![1, 4],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::AckVerifyFirmware {
                    result: true,
                    hash: [0; 32],
                }),
                vec![
                    4, 1, 5, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0,
                ],
                vec![
                    1, 5, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::NackWithIdx { block_idx: 0xFFFFFFFF }),
                vec![8, 1, 6, 255, 255, 255, 255, 15, 0],
                vec![1, 6, 255, 255, 255, 255, 15],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::AckWithIdx { block_idx: 0xFFFFFFFF }),
                vec![8, 1, 7, 255, 255, 255, 255, 15, 0],
                vec![1, 7, 255, 255, 255, 255, 15],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::AckWithIdxCrc {
                    block_idx: 0xFFFFFFFF,
                    crc: 0xFFFFFFFF,
                }),
                vec![13, 1, 8, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 0],
                vec![1, 8, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::WriteFirmwareBlock {
                    block_idx: 0xFFFFFFFF,
                    block_data: &[0xFF; 256],
                }),
                vec![
                    255, 1, 9, 255, 255, 255, 255, 15, 128, 2, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 12, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 0,
                ],
                vec![
                    1, 9, 255, 255, 255, 255, 15, 128, 2, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                ],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::FirmwareOutOfBounds { block_idx: 0xFFFFFFFF }),
                vec![8, 1, 10, 255, 255, 255, 255, 15, 0],
                vec![1, 10, 255, 255, 255, 255, 15],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::NoCosignHeader),
                vec![3, 1, 11, 0],
                vec![1, 11],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::AckFirmwareVersion { version: "v1.2.3" }),
                vec![10, 1, 13, 6, 118, 49, 46, 50, 46, 51, 0],
                vec![1, 13, 6, 118, 49, 46, 50, 46, 51],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::AckBootloaderVersion {
                    version: "v1.2.3-longversionstring",
                }),
                vec![
                    28, 1, 15, 24, 118, 49, 46, 50, 46, 51, 45, 108, 111, 110, 103, 118, 101, 114, 115, 105, 111, 110, 115, 116, 114, 105,
                    110, 103, 0,
                ],
                vec![
                    1, 15, 24, 118, 49, 46, 50, 46, 51, 45, 108, 111, 110, 103, 118, 101, 114, 115, 105, 111, 110, 115, 116, 114, 105, 110,
                    103,
                ],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::FirmwareVersion),
                vec![3, 1, 12, 0],
                vec![1, 12],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::BootloaderVersion),
                vec![3, 1, 14, 0],
                vec![1, 14],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::ChallengeSet { secret: [0xFFFFFFFF; 8] }),
                vec![
                    43, 1, 16, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255,
                    255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 0,
                ],
                vec![
                    1, 16, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255,
                    255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15,
                ],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::AckChallengeSet {
                    result: SecretSaveResponse::Error,
                }),
                vec![4, 1, 17, 2, 0],
                vec![1, 17, 2],
            ),
            (
                HostProtocolMessage::Bootloader(Bootloader::BootFirmware),
                vec![3, 1, 18, 0],
                vec![1, 18],
            ),
        ];

        println!("----------------------------------");
        println!("Bootloader messages data after encoding:");
        println!("----------------------------------");
        for (msg, wanted_cobs, wanted) in data_bootloader_messages {
            println!(" {:02x?}", msg);
            // make sure these encoded messages stay the same for backward compatibility
            assert_eq!(encoded_data_cobs(msg.clone()), wanted_cobs);
            // make sure these encoded messages stay the same for backward compatibility
            assert_eq!(encoded_data(msg), wanted);
        }
    }

    #[test]
    fn check_bluetooth_messages() {
        let data_bluetooth_messages = [
            (
                HostProtocolMessage::Bluetooth(Bluetooth::DisableChannels(AdvChan::C37 | AdvChan::C38)),
                vec![1, 1, 2, 96, 0],
                vec![0, 0, 96],
            ),
            (
                HostProtocolMessage::Bluetooth(Bluetooth::DisableChannels(AdvChan::C37 | AdvChan::C39)),
                vec![1, 1, 2, 160, 0],
                vec![0, 0, 160],
            ),
            (
                HostProtocolMessage::Bluetooth(Bluetooth::DisableChannels(AdvChan::C38 | AdvChan::C39)),
                vec![1, 1, 2, 192, 0],
                vec![0, 0, 192],
            ),
            (
                HostProtocolMessage::Bluetooth(Bluetooth::AckDisableChannels),
                vec![1, 2, 1, 0],
                vec![0, 1],
            ),
            (
                HostProtocolMessage::Bluetooth(Bluetooth::NackDisableChannels),
                vec![1, 2, 2, 0],
                vec![0, 2],
            ),
            (HostProtocolMessage::Bluetooth(Bluetooth::Enable), vec![1, 2, 3, 0], vec![0, 3]),
            (HostProtocolMessage::Bluetooth(Bluetooth::AckEnable), vec![1, 2, 4, 0], vec![0, 4]),
            (HostProtocolMessage::Bluetooth(Bluetooth::Disable), vec![1, 2, 5, 0], vec![0, 5]),
            (HostProtocolMessage::Bluetooth(Bluetooth::AckDisable), vec![1, 2, 6, 0], vec![0, 6]),
            (
                HostProtocolMessage::Bluetooth(Bluetooth::GetSignalStrength),
                vec![1, 2, 7, 0],
                vec![0, 7],
            ),
            (
                HostProtocolMessage::Bluetooth(Bluetooth::SignalStrength(Some(i8::MAX))),
                vec![1, 4, 8, 1, 127, 0],
                vec![0, 8, 1, 127],
            ),
            (
                HostProtocolMessage::Bluetooth(Bluetooth::SendData(&[0xFF; 256])),
                vec![
                    1, 255, 9, 128, 2, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 6, 255, 255, 255, 255, 255, 0,
                ],
                vec![
                    0, 9, 128, 2, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                ],
            ),
            (
                HostProtocolMessage::Bluetooth(Bluetooth::SendDataResponse(SendDataResponse::Sent)),
                vec![1, 2, 10, 1, 0],
                vec![0, 10, 0],
            ),
            (
                HostProtocolMessage::Bluetooth(Bluetooth::GetReceivedData),
                vec![1, 2, 11, 0],
                vec![0, 11],
            ),
            (
                HostProtocolMessage::Bluetooth(Bluetooth::ReceivedData(&[0xFF; 256])),
                vec![
                    1, 255, 12, 128, 2, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 6, 255, 255, 255, 255, 255, 0,
                ],
                vec![
                    0, 12, 128, 2, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                ],
            ),
            (
                HostProtocolMessage::Bluetooth(Bluetooth::NoReceivedData),
                vec![1, 2, 13, 0],
                vec![0, 13],
            ),
            (
                HostProtocolMessage::Bluetooth(Bluetooth::GetFirmwareVersion),
                vec![1, 2, 14, 0],
                vec![0, 14],
            ),
            (
                HostProtocolMessage::Bluetooth(Bluetooth::AckFirmwareVersion { version: "v1.2.3" }),
                vec![1, 9, 15, 6, 118, 49, 46, 50, 46, 51, 0],
                vec![0, 15, 6, 118, 49, 46, 50, 46, 51],
            ),
            (
                HostProtocolMessage::Bluetooth(Bluetooth::GetBtAddress),
                vec![1, 2, 16, 0],
                vec![0, 16],
            ),
            (
                HostProtocolMessage::Bluetooth(Bluetooth::AckBtAddress { bt_address: [0xFF; 6] }),
                vec![1, 8, 17, 255, 255, 255, 255, 255, 255, 0],
                vec![0, 17, 255, 255, 255, 255, 255, 255],
            ),
        ];

        println!("----------------------------------");
        println!("Bluetooth messages data after encoding:");
        println!("----------------------------------");
        for (msg, wanted_cobs, wanted) in data_bluetooth_messages {
            println!("{:02x?}", msg);
            // make sure these encoded messages stay the same for backward compatibility
            assert_eq!(encoded_data_cobs(msg.clone()), wanted_cobs);
            // make sure these encoded messages stay the same for backward compatibility
            assert_eq!(encoded_data(msg), wanted);
        }
    }
}
