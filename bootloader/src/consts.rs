// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{BASE_APP_ADDR, BASE_BOOTLOADER_ADDR};
#[cfg(not(feature = "debug"))]
use consts_global::SIGNATURE_HEADER_SIZE;

#[used]
/// Start address of the bootloader in flash memory, stored in UICR
/// The bootloader address is stored in a dedicated UICR register to allow the device
/// to locate and execute the bootloader during startup. The UICR (User Information
/// Configuration Registers) provide non-volatile storage for critical system parameters.
#[link_section = ".mbr_uicr_bootloader_addr"]
pub static MBR_UICR_BOOTLOADER_ADDR: u32 = BASE_BOOTLOADER_ADDR;

#[cfg(not(feature = "debug"))]
#[used]
#[link_section = ".uicr_approtect"]
pub static APPROTECT: u32 = 0x0000_0000;

/// Base address of the interrupt vector table for signed firmware
/// When booting signed firmware, the interrupt vector table is placed after SIGNATURE_HEADER_SIZE,
/// which is after the SoftDevice but before the application code. This ensures proper
/// interrupt handling while maintaining the security of the signed firmware.
#[cfg(not(feature = "debug"))]
pub const INT_VECTOR_TABLE_BASE: u32 = BASE_APP_ADDR + SIGNATURE_HEADER_SIZE;

/// Base address of the interrupt vector table for unsigned firmware
/// Points to the SoftDevice base address at 0x1000, which is where the interrupt vector table
/// must be located for unsigned firmware to properly handle interrupts through the SoftDevice
#[cfg(not(not(feature = "debug")))]
pub const INT_VECTOR_TABLE_BASE: u32 = 0x1000;

/// Size of the application area in flash memory
/// This constant defines the maximum size available for the application firmware.
/// Starting from BASE_APP_ADDR up to BASE_BOOTLOADER_ADDR
/// consider that a header is needed for cosign2 signature so real fw app goes from
/// BASE_APP_ADDR + SIGNATURE_HEADER_SIZE to BASE_BOOTLOADER_ADDR
pub const APP_SIZE: u32 = BASE_BOOTLOADER_ADDR - BASE_APP_ADDR;

/// Size of a flash memory page in bytes (4KB)
/// This constant defines the size of a single flash memory page on the nRF52 microcontroller.
/// Flash memory is organized into pages that can be erased and written independently.
/// The page size is important for flash operations as they must be aligned to page boundaries.
pub const FLASH_PAGE: u32 = 4096;

/// Index of the UICR register used to store the SEALED_SECRET value (0x5A5A5A5A)
/// This register is checked to determine if a secret has been properly sealed in UICR memory.
/// The value of 8 corresponds to UICR register 40 (32 + 8), which follows the 8 registers
/// used for storing the actual secret value.
pub const SEAL_IDX: usize = 8;

/// Magic value used to verify sealing of the challenge-response secret in UICR memory.
/// When a secret is written to UICR, this value is written to SEAL_IDX to indicate
/// that the secret has been properly sealed and cannot be overwritten. The value
/// 0x5A5A5A5A is chosen as a recognizable pattern that is unlikely to occur randomly.
pub const SEALED_SECRET: u32 = 0x5A5A5A5A;
