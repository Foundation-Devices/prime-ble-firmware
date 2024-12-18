// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
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
    /// BLE radio enabled
    AckEnable,
    /// Turn off the BLE radio
    Disable,
    /// BLE radio disabled
    AckDisable,

    /// Request current signal strength
    GetSignalStrength,
    /// Response with signal strength value (0-255)
    SignalStrength(u8),

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
extern crate std;
#[test]
fn calculate_bootloader_message_sizes() {
    use postcard::to_slice_cobs;
    use std::println;

    // Helper function to calculate COBS size
    fn get_cobs_size(msg: HostProtocolMessage) -> usize {
        let mut buf = [0; 512]; // Buffer large enough for all messages
        let serialized = to_slice_cobs(&msg, &mut buf).unwrap();
        serialized.len()
    }

    // Test each variant
    let sizes_bootloader_cobs_sent = [
        (
            "AckEraseFirmware",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::AckEraseFirmware)),
        ),
        (
            "AckVerifyFirmware",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::AckVerifyFirmware {
                result: true,
                hash: [0; 32],
            })),
        ),
        (
            "NackWithIdx",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::NackWithIdx { block_idx: 0xFFFFFFFF })),
        ),
        (
            "AckWithIdx",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::AckWithIdx { block_idx: 0xFFFFFFFF })),
        ),
        (
            "AckWithIdxCrc",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::AckWithIdxCrc {
                block_idx: 0xFFFFFFFF,
                crc: 0xFFFFFFFF,
            })),
        ),
        (
            "FirmwareOutOfBounds",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::FirmwareOutOfBounds {
                block_idx: 0xFFFFFFFF,
            })),
        ),
        (
            "NoCosignHeader",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::NoCosignHeader)),
        ),
        (
            "AckFirmwareVersion",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::AckFirmwareVersion {
                version: "v1.2.3",
            })),
        ),
        (
            "AckBootloaderVersion",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::AckBootloaderVersion {
                version: "v1.2.3-longversionstring",
            })),
        ),
        (
            "AckChallengeSet",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::AckChallengeSet {
                result: SecretSaveResponse::Error,
            })),
        ),
    ];

    // Test each variant
    let sizes_bootloader_cobs_recv = [
        (
            "EraseFirmware",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::EraseFirmware)),
        ),
        (
            "VerifyFirmware",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::VerifyFirmware)),
        ),
        // Test WriteFirmwareBlock with different sizes
        (
            "WriteFirmwareBlock(256)",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::WriteFirmwareBlock {
                block_idx: 0xFFFFFFFF,
                block_data: &[0xFF; 256],
            })),
        ),
        (
            "FirmwareVersion",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::FirmwareVersion)),
        ),
        (
            "BootloaderVersion",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::BootloaderVersion)),
        ),
        (
            "ChallengeSet",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::ChallengeSet {
                secret: [0xFFFFFFFF; 8],
            })),
        ),
        (
            "BootFirmware",
            get_cobs_size(HostProtocolMessage::Bootloader(Bootloader::BootFirmware)),
        ),
    ];

    // Add new test array for Bluetooth messages
    let sizes_bluetooth_messages = [
        ("Enable", get_cobs_size(HostProtocolMessage::Bluetooth(Bluetooth::Enable))),
        ("Disable", get_cobs_size(HostProtocolMessage::Bluetooth(Bluetooth::Disable))),
        (
            "GetSignalStrength",
            get_cobs_size(HostProtocolMessage::Bluetooth(Bluetooth::GetSignalStrength)),
        ),
        (
            "SignalStrength",
            get_cobs_size(HostProtocolMessage::Bluetooth(Bluetooth::SignalStrength(255))),
        ),
        (
            "SendData(256)",
            get_cobs_size(HostProtocolMessage::Bluetooth(Bluetooth::SendData(&[0xFF; 256]))),
        ),
        (
            "ReceivedData(256)",
            get_cobs_size(HostProtocolMessage::Bluetooth(Bluetooth::ReceivedData(&[0xFF; 256]))),
        ),
        (
            "GetFirmwareVersion",
            get_cobs_size(HostProtocolMessage::Bluetooth(Bluetooth::GetFirmwareVersion)),
        ),
        (
            "AckFirmwareVersion",
            get_cobs_size(HostProtocolMessage::Bluetooth(Bluetooth::AckFirmwareVersion { version: "v1.2.3" })),
        ),
        (
            "GetBtAddress",
            get_cobs_size(HostProtocolMessage::Bluetooth(Bluetooth::GetBtAddress)),
        ),
        (
            "AckBtAddress",
            get_cobs_size(HostProtocolMessage::Bluetooth(Bluetooth::AckBtAddress { bt_address: [0xFF; 6] })),
        ),
    ];

    // Print results sorted by size
    let mut sizes_vec = sizes_bootloader_cobs_recv.to_vec();
    sizes_vec.sort_by_key(|(_name, size)| *size);

    println!("Message sizes after COBS encoding of bootloader received messages:");
    println!("----------------------------------");
    for (name, size) in sizes_vec {
        println!("{}: {} bytes", name, size);
    }

    // Print results sorted by size
    let mut sizes_vec = sizes_bootloader_cobs_sent.to_vec();
    sizes_vec.sort_by_key(|(_name, size)| *size);

    println!("Message sizes after COBS encoding of bootloader sent messages:");
    println!("----------------------------------");
    for (name, size) in sizes_vec {
        println!("{}: {} bytes", name, size);
    }

    // Add printing for Bluetooth message sizes
    let mut bluetooth_sizes = sizes_bluetooth_messages.to_vec();
    bluetooth_sizes.sort_by_key(|(_name, size)| *size);

    println!("\nMessage sizes after COBS encoding of Bluetooth messages:");
    println!("----------------------------------");
    for (name, size) in bluetooth_sizes {
        println!("{}: {} bytes", name, size);
    }
}
