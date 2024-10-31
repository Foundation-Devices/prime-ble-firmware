use crate::nus::NUS_UUID;

pub const MTU: usize = 256;
pub const ATT_MTU: usize = MTU + 3;

pub const DEVICE_NAME: &str = "Passport Prime";
pub const SHORT_NAME: &str = "Prime";

pub const SERVICES_LIST: [[u8; 16]; 1] = [NUS_UUID.to_le_bytes()];

// Buffer max bt packets
pub const BT_MAX_NUM_PKT: usize = 4;

// Address and length of secret saved
pub const UICR_SECRET_START: u32 = 0x10001080;
pub const UICR_SECRET_SIZE: u32 = 0x10;
