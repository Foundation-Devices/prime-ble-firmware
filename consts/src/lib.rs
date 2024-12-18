// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]

/// Application Maximum Transfer Unit (MTU) size for BLE communication.
/// This defines the maximum size of a single BLE data packet that can be sent.
#[cfg(feature = "dle")]
/// Set to 244 bytes to allow efficient data transfer while staying within BLE limits.
pub const APP_MTU: usize = 244;
#[cfg(not(feature = "dle"))]
/// Set to 256 bytes to allow normally-sized data transfer.
pub const APP_MTU: usize = 256;

/// ATT MTU size includes 3 bytes of overhead on top of base MTU.
/// The ATT protocol adds a 3-byte header to each MTU-sized packet:
/// - 1 byte for Attribute Opcode
/// - 2 bytes for Attribute Handle
pub const ATT_MTU: usize = APP_MTU + 3;

/// Full device name advertised over BLE.
/// This is the complete name that will appear when scanning for the device.
/// Used in scan response data since it's longer than the short name.
pub const DEVICE_NAME: &str = "Passport Prime";

/// Short device name used in limited advertising data.
/// A shorter version of the device name used in the initial advertising packet
/// to stay within the 31-byte advertising data size limit.
pub const SHORT_NAME: &str = "Prime";

/// UUID for the Nordic UART Service (NUS).
pub const NUS_UUID: u128 = 0x6E400001_B5A3_F393_E0A9_E50E24DCCA9E;

/// List of BLE service UUIDs supported by this device.
/// Currently only includes the Nordic UART Service (NUS).
pub const SERVICES_LIST: [[u8; 16]; 1] = [NUS_UUID.to_le_bytes()];

/// Maximum number of BLE packets that can be buffered.
/// This limits memory usage while ensuring reliable data transfer.
pub const BT_MAX_NUM_PKT: usize = 4;

/// Starting address in UICR (User Information Configuration Registers) where the device secret is stored.
/// UICR is non-volatile memory that persists across resets and firmware updates.
/// This secret is used for challenge-response authentication between the MPU and this device.
/// The secret can only be written once and cannot be overwritten for security.
pub const UICR_SECRET_START: u32 = 0x10001080;

/// Size in bytes of the secret stored in UICR.
/// 0x20 = 32 bytes = 256 bits
/// This size matches the output length of HMAC-SHA256 used for authentication.
pub const UICR_SECRET_SIZE: u32 = 0x20;
