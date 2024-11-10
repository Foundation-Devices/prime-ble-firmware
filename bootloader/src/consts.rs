#[used]
/// Start address of the bootloader in flash memory, stored in UICR
/// This address (0x26000) is stored in a dedicated UICR register to allow the device
/// to locate and execute the bootloader during startup. The UICR (User Information
/// Configuration Registers) provide non-volatile storage for critical system parameters.
#[link_section = ".uicr_bootloader_start_address"]
pub static BOOTLOADER_ADDR: u32 = 0x26000;

/// Base address of the interrupt vector table for signed firmware
/// When booting signed firmware, the interrupt vector table is placed at 0x19800,
/// which is after the SoftDevice but before the application code. This ensures proper
/// interrupt handling while maintaining the security of the signed firmware.
#[cfg(feature = "boot-signed-fw")]
pub const INT_VECTOR_TABLE_BASE: u32 = 0x19800;

/// Base address of the interrupt vector table for unsigned firmware
/// Points to the SoftDevice base address at 0x1000, which is where the interrupt vector table
/// must be located for unsigned firmware to properly handle interrupts through the SoftDevice
#[cfg(feature = "boot-unsigned-fw")]
pub const INT_VECTOR_TABLE_BASE: u16 = 0x1000;

/// Base address for the application in flash memory
/// This is where the actual application firmware code begins in flash memory at 0x19000,
/// after the SoftDevice and before the bootloader region
pub const BASE_APP_ADDR: u32 = 0x19000;

/// Size of the application area in flash memory (50KB)
/// This constant defines the maximum size available for the application firmware.
/// The size is set to 50KB (0xD000 bytes) to leave sufficient space for the bootloader
/// while allowing reasonably sized application code.
pub const APP_SIZE: u32 = 0xD000;

/// Start address of the bootloader application code in flash memory
/// This address (0x26000) marks where the bootloader code begins in flash.
/// It must match the address specified in BOOTLOADER_ADDR above to ensure
/// proper execution of the bootloader during device startup.
pub const BASE_BOOTLOADER_APP: u32 = 0x26000;

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

/// Start address of secret storage in UICR region
/// This constant defines the base address in UICR memory where the challenge-response
/// authentication secret is stored. The UICR (User Information Configuration Registers)
/// region starts at 0x10001000, and this secret storage begins at offset 0x80 (register 32).
/// The secret uses 8 consecutive 32-bit UICR registers starting from this address.
pub const UICR_SECRET_START: u32 = 0x10001080;

/// Size of the secret storage area in UICR (32 bytes)
/// This constant defines the size of the storage area in UICR memory reserved for storing
/// the challenge-response authentication secret. The size is 32 bytes (0x20) which matches
/// the 8 UICR registers used (8 registers * 4 bytes per register = 32 bytes total).
pub const UICR_SECRET_SIZE: u8 = 0x20;

#[cfg(feature = "no-dbg-access")]
#[used]
#[link_section = ".uicr_appprotection"]
pub static APP_PROTECTION: u8 = 0x00;
