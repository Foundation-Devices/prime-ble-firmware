// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bootloader flash write bounds validation.

use consts_global::{BASE_APP_ADDR, BASE_BOOTLOADER_ADDR};

/// Size of the firmware update chunks sent to the bootloader.
pub const FIRMWARE_BLOCK_SIZE: usize = 256;

/// Minimum write alignment for nRF52 flash writes.
pub const FLASH_WRITE_SIZE: usize = 4;

/// Returns the exclusive end address for an application flash range.
pub fn checked_app_flash_range_end(address: u32, len: u32) -> Option<u32> {
    if address < BASE_APP_ADDR || address >= BASE_BOOTLOADER_ADDR {
        return None;
    }

    let end = address.checked_add(len)?;
    if end <= BASE_BOOTLOADER_ADDR {
        Some(end)
    } else {
        None
    }
}

/// Validates a bootloader firmware write before touching NVMC.
pub fn is_app_flash_write_in_bounds(address: u32, len: usize) -> bool {
    let Ok(len_u32) = u32::try_from(len) else {
        return false;
    };

    let is_aligned = address % (FLASH_WRITE_SIZE as u32) == 0 && len % FLASH_WRITE_SIZE == 0;
    if len == 0 || !is_aligned {
        return false;
    }

    let Some(end) = checked_app_flash_range_end(address, len_u32) else {
        return false;
    };

    len == FIRMWARE_BLOCK_SIZE || end == BASE_BOOTLOADER_ADDR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_allows_write_ending_at_bootloader_boundary() {
        assert_eq!(
            checked_app_flash_range_end(BASE_BOOTLOADER_ADDR - FIRMWARE_BLOCK_SIZE as u32, FIRMWARE_BLOCK_SIZE as u32),
            Some(BASE_BOOTLOADER_ADDR)
        );
    }

    #[test]
    fn range_rejects_write_crossing_bootloader_boundary_by_one_byte() {
        assert_eq!(
            checked_app_flash_range_end(BASE_BOOTLOADER_ADDR - FIRMWARE_BLOCK_SIZE as u32 + 1, FIRMWARE_BLOCK_SIZE as u32),
            None
        );
    }

    #[test]
    fn write_rejects_start_at_bootloader_boundary() {
        assert!(!is_app_flash_write_in_bounds(BASE_BOOTLOADER_ADDR, FIRMWARE_BLOCK_SIZE));
    }

    #[test]
    fn write_rejects_zero_length_blocks() {
        assert!(!is_app_flash_write_in_bounds(BASE_APP_ADDR, 0));
    }

    #[test]
    fn write_rejects_oversized_blocks() {
        assert!(!is_app_flash_write_in_bounds(BASE_APP_ADDR, FIRMWARE_BLOCK_SIZE + FLASH_WRITE_SIZE));
    }

    #[test]
    fn write_rejects_unaligned_start_addresses() {
        assert!(!is_app_flash_write_in_bounds(BASE_APP_ADDR + 1, FLASH_WRITE_SIZE));
    }

    #[test]
    fn write_rejects_unaligned_lengths() {
        assert!(!is_app_flash_write_in_bounds(BASE_APP_ADDR, FLASH_WRITE_SIZE - 1));
    }

    #[test]
    fn write_allows_normal_blocks() {
        assert!(is_app_flash_write_in_bounds(BASE_APP_ADDR, FIRMWARE_BLOCK_SIZE));
    }

    #[test]
    fn write_allows_final_partial_block_at_bootloader_boundary() {
        assert!(is_app_flash_write_in_bounds(
            BASE_BOOTLOADER_ADDR - FLASH_WRITE_SIZE as u32,
            FLASH_WRITE_SIZE
        ));
    }
}
