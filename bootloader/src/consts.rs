#[used]
#[link_section = ".uicr_bootloader_start_address"]
pub static BOOTLOADER_ADDR: u32 = 0x26000;

#[cfg(feature = "boot-signed-fw")]
pub const BASE_ADDRESS_APP: u32 = 0x19800;

#[cfg(feature = "boot-unsigned-fw")]
pub const BASE_ADDRESS_APP: u32 = 0x1000; // SD base address

pub const BASE_APP_ADDR: u32 = 0x19000;
pub const APP_SIZE: u32 = 0xC800; // 50K
pub const BASE_BOOTLOADER_APP: u32 = 0x26000;
pub const FLASH_PAGE: u32 = 4096;
pub const SEAL_IDX: usize = 5;
pub const SEALED_SECRET: u32 = 0x5A5A5A5A;
pub const UICR_SECRET_START: u32 = 0x10001080;
pub const UICR_SECRET_SIZE: u32 = 0x10;



#[cfg(feature = "no-dbg-access")]
#[used]
#[link_section = ".uicr_appprotection"]
pub static APP_PROTECTION: i32 = 0x00;
