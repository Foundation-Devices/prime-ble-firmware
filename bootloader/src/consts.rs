#[used]
/// Start address of the bootloader in flash memory, stored in UICR
#[link_section = ".uicr_bootloader_start_address"]
pub static BOOTLOADER_ADDR: u32 = 0x26000;

/// Base address of the interrupt vector table for signed firmware
#[cfg(feature = "boot-signed-fw")]
pub const INT_VECTOR_TABLE_BASE: u32 = 0x19800;

/// Base address of the interrupt vector table for unsigned firmware
/// Points to the SoftDevice base address
#[cfg(feature = "boot-unsigned-fw")]
pub const INT_VECTOR_TABLE_BASE: u32 = 0x1000;

/// Base address for the application in flash memory
pub const BASE_APP_ADDR: u32 = 0x19000;

/// Size of the application area in flash (50KB)
pub const APP_SIZE: u32 = 0xC800;

/// Start address of the bootloader application code
pub const BASE_BOOTLOADER_APP: u32 = 0x26000;

/// Size of a flash memory page in bytes
pub const FLASH_PAGE: u32 = 4096;

/// Index used for sealing operations
pub const SEAL_IDX: usize = 5;

/// Magic value used to verify sealing
pub const SEALED_SECRET: u32 = 0x5A5A5A5A;

/// Start address of secret storage in UICR region
pub const UICR_SECRET_START: u32 = 0x10001080;

/// Size of the secret storage area in UICR (16 bytes)
pub const UICR_SECRET_SIZE: u32 = 0x10;

#[cfg(feature = "no-dbg-access")]
#[used]
#[link_section = ".uicr_appprotection"]
pub static APP_PROTECTION: i32 = 0x00;
