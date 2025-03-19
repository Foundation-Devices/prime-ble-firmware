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
/// Make sure to only append new messages at the end of the enum, to keep forward compatibility
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

/// Bootloader-specific messages for firmware updates and verification.
///
/// Make sure to only append new messages at the end of the enum, to keep forward compatibility
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
/// Make sure to only append new messages at the end of the enum, to keep forward compatibility
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

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use postcard::to_slice_cobs;
    use std::println;
    use std::{vec, vec::Vec};

    fn encoded_data(msg: HostProtocolMessage) -> Vec<u8> {
        let mut buf = [0u8; 512]; // Buffer large enough for all messages
        let serialized = to_slice_cobs(&msg, &mut buf).unwrap();
        serialized.to_vec()
    }

    #[test]
    fn check_base_messages() {
        // Test each variant
        let data_base_messages = [
            ("Reset", encoded_data(HostProtocolMessage::Reset), Some(vec![2, 2, 0])),
            ("GetState", encoded_data(HostProtocolMessage::GetState), Some(vec![2, 3, 0])),
            (
                "AckState(State::Disabled)",
                encoded_data(HostProtocolMessage::AckState(State::Disabled)),
                Some(vec![3, 4, 1, 0]),
            ),
            (
                "AckState(State::Enabled)",
                encoded_data(HostProtocolMessage::AckState(State::Enabled)),
                Some(vec![2, 4, 1, 0]),
            ),
            (
                "AckState(State::FirmwareUpgrade)",
                encoded_data(HostProtocolMessage::AckState(State::FirmwareUpgrade)),
                Some(vec![3, 4, 2, 0]),
            ),
            (
                "AckState(State::Unknown)",
                encoded_data(HostProtocolMessage::AckState(State::Unknown)),
                Some(vec![3, 4, 3, 0]),
            ),
            (
                "ChallengeRequest",
                encoded_data(HostProtocolMessage::ChallengeRequest { nonce: 0 }),
                Some(vec![2, 5, 1, 0]),
            ),
            (
                "ChallengeResult",
                encoded_data(HostProtocolMessage::ChallengeResult { result: [0u8; 32] }),
                Some(vec![
                    2, 6, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0,
                ]),
            ),
            (
                "PostcardError(PostcardError::Deser)",
                encoded_data(HostProtocolMessage::PostcardError(PostcardError::Deser)),
                Some(vec![2, 7, 1, 0]),
            ),
            (
                "PostcardError(PostcardError::OverFull)",
                encoded_data(HostProtocolMessage::PostcardError(PostcardError::OverFull)),
                Some(vec![3, 7, 1, 0]),
            ),
            (
                "InappropriateMessage(State::Disabled)",
                encoded_data(HostProtocolMessage::InappropriateMessage(State::Disabled)),
                Some(vec![3, 8, 1, 0]),
            ),
            (
                "InappropriateMessage(State::Enabled)",
                encoded_data(HostProtocolMessage::InappropriateMessage(State::Enabled)),
                Some(vec![2, 8, 1, 0]),
            ),
            (
                "InappropriateMessage(State::FirmwareUpgrade)",
                encoded_data(HostProtocolMessage::InappropriateMessage(State::FirmwareUpgrade)),
                Some(vec![3, 8, 2, 0]),
            ),
            (
                "InappropriateMessage(State::Unknown)",
                encoded_data(HostProtocolMessage::InappropriateMessage(State::Unknown)),
                Some(vec![3, 8, 3, 0]),
            ),
        ];

        println!("----------------------------------");
        println!("Base messages data after encoding:");
        println!("----------------------------------");
        for (name, encoded, wanted) in data_base_messages {
            println!("{}: {:02x?}", name, encoded);
            if let Some(wanted) = wanted {
                // make sure these encoded messages stay the same for foreward compatibility
                assert_eq!(encoded, wanted);
            }
        }
    }

    #[test]
    fn check_bootloader_messages() {
        // Test each variant
        let data_bootloader_messages = [
            (
                "AckEraseFirmware",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::AckEraseFirmware)),
                Some(vec![3, 1, 1, 0]),
            ),
            (
                "AckVerifyFirmware",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::AckVerifyFirmware {
                    result: true,
                    hash: [0; 32],
                })),
                Some(vec![
                    4, 1, 5, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0,
                ]),
            ),
            (
                "NackWithIdx",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::NackWithIdx { block_idx: 0xFFFFFFFF })),
                Some(vec![8, 1, 6, 255, 255, 255, 255, 15, 0]),
            ),
            (
                "AckWithIdx",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::AckWithIdx { block_idx: 0xFFFFFFFF })),
                Some(vec![8, 1, 7, 255, 255, 255, 255, 15, 0]),
            ),
            (
                "AckWithIdxCrc",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::AckWithIdxCrc {
                    block_idx: 0xFFFFFFFF,
                    crc: 0xFFFFFFFF,
                })),
                Some(vec![13, 1, 8, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 0]),
            ),
            (
                "FirmwareOutOfBounds",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::FirmwareOutOfBounds {
                    block_idx: 0xFFFFFFFF,
                })),
                Some(vec![8, 1, 10, 255, 255, 255, 255, 15, 0]),
            ),
            (
                "NoCosignHeader",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::NoCosignHeader)),
                Some(vec![3, 1, 11, 0]),
            ),
            (
                "AckFirmwareVersion",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::AckFirmwareVersion {
                    version: "v1.2.3",
                })),
                Some(vec![10, 1, 13, 6, 118, 49, 46, 50, 46, 51, 0]),
            ),
            (
                "AckBootloaderVersion",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::AckBootloaderVersion {
                    version: "v1.2.3-longversionstring",
                })),
                Some(vec![
                    28, 1, 15, 24, 118, 49, 46, 50, 46, 51, 45, 108, 111, 110, 103, 118, 101, 114, 115, 105, 111, 110, 115, 116, 114, 105,
                    110, 103, 0,
                ]),
            ),
            (
                "AckChallengeSet",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::AckChallengeSet {
                    result: SecretSaveResponse::Error,
                })),
                Some(vec![4, 1, 17, 2, 0]),
            ),
            (
                "EraseFirmware",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::EraseFirmware)),
                Some(vec![2, 1, 1, 0]),
            ),
            (
                "WriteFirmwareBlock(256)",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::WriteFirmwareBlock {
                    block_idx: 0xFFFFFFFF,
                    block_data: &[0xFF; 256],
                })),
                Some(vec![
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
                ]),
            ),
            (
                "FirmwareVersion",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::FirmwareVersion)),
                Some(vec![3, 1, 12, 0]),
            ),
            (
                "BootloaderVersion",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::BootloaderVersion)),
                Some(vec![3, 1, 14, 0]),
            ),
            (
                "ChallengeSet",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::ChallengeSet {
                    secret: [0xFFFFFFFF; 8],
                })),
                Some(vec![
                    43, 1, 16, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255,
                    255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15, 0,
                ]),
            ),
            (
                "BootFirmware",
                encoded_data(HostProtocolMessage::Bootloader(Bootloader::BootFirmware)),
                Some(vec![3, 1, 18, 0]),
            ),
        ];

        println!("----------------------------------");
        println!("Bootloader messages data after encoding:");
        println!("----------------------------------");
        for (name, encoded, wanted) in data_bootloader_messages {
            println!("{}: {:02x?}", name, encoded);
            if let Some(wanted) = wanted {
                // make sure these encoded messages stay the same for foreward compatibility
                assert_eq!(encoded, wanted);
            }
        }
    }

    #[test]
    fn check_bluetooth_messages() {
        let data_bluetooth_messages = [
            (
                "DisableChannels(37|38)",
                encoded_data(HostProtocolMessage::Bluetooth(Bluetooth::DisableChannels(
                    AdvChan::C37 | AdvChan::C38,
                ))),
                None,
            ),
            (
                "DisableChannels(37|39)",
                encoded_data(HostProtocolMessage::Bluetooth(Bluetooth::DisableChannels(
                    AdvChan::C37 | AdvChan::C39,
                ))),
                None,
            ),
            (
                "DisableChannels(38|39)",
                encoded_data(HostProtocolMessage::Bluetooth(Bluetooth::DisableChannels(
                    AdvChan::C38 | AdvChan::C39,
                ))),
                None,
            ),
            (
                "Enable",
                encoded_data(HostProtocolMessage::Bluetooth(Bluetooth::Enable)),
                Some(vec![1, 2, 3, 0]),
            ),
            (
                "Disable",
                encoded_data(HostProtocolMessage::Bluetooth(Bluetooth::Disable)),
                Some(vec![1, 2, 5, 0]),
            ),
            (
                "GetSignalStrength",
                encoded_data(HostProtocolMessage::Bluetooth(Bluetooth::GetSignalStrength)),
                None,
            ),
            (
                "SignalStrength",
                encoded_data(HostProtocolMessage::Bluetooth(Bluetooth::SignalStrength(Some(i8::MAX)))),
                None,
            ),
            (
                "SendData(256)",
                encoded_data(HostProtocolMessage::Bluetooth(Bluetooth::SendData(&[0xFF; 256]))),
                None,
            ),
            (
                "ReceivedData(256)",
                encoded_data(HostProtocolMessage::Bluetooth(Bluetooth::ReceivedData(&[0xFF; 256]))),
                None,
            ),
            (
                "GetFirmwareVersion",
                encoded_data(HostProtocolMessage::Bluetooth(Bluetooth::GetFirmwareVersion)),
                None,
            ),
            (
                "AckFirmwareVersion",
                encoded_data(HostProtocolMessage::Bluetooth(Bluetooth::AckFirmwareVersion { version: "v1.2.3" })),
                None,
            ),
            (
                "GetBtAddress",
                encoded_data(HostProtocolMessage::Bluetooth(Bluetooth::GetBtAddress)),
                None,
            ),
            (
                "AckBtAddress",
                encoded_data(HostProtocolMessage::Bluetooth(Bluetooth::AckBtAddress { bt_address: [0xFF; 6] })),
                None,
            ),
        ];

        println!("----------------------------------");
        println!("Bluetooth messages data after encoding:");
        println!("----------------------------------");
        for (name, encoded, wanted) in data_bluetooth_messages {
            println!("{}: {:02x?}", name, encoded);
            if let Some(wanted) = wanted {
                // make sure these encoded messages stay the same for foreward compatibility
                assert_eq!(encoded, wanted);
            }
        }
    }
}
