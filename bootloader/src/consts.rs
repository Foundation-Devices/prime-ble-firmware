#[used]
#[link_section = ".uicr_bootloader_start_address"]
pub static BOOTLOADER_ADDR: u32 = 0x27000;

#[cfg(feature = "boot-signed-fw")]
pub const BASE_ADDRESS_APP: u32 = 0x19800;

#[cfg(feature = "boot-unsigned-fw")]
pub const BASE_ADDRESS_APP: u32 = 0x1000; // SD base address

pub const BASE_FLASH_ADDR: u32 = 0x19000;
pub const BASE_BOOTLOADER_APP: u32 = 0x27000;
pub const FLASH_PAGE: u32 = 4096;
pub const HEADER_SIZE: u32 = 0x800;

#[cfg(feature = "no-dbg-access")]
#[used]
#[link_section = ".uicr_appprotection"]
pub static APP_PROTECTION: i32 = 0x00;
