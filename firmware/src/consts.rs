use crate::nus::NUS_UUID;

pub const MTU: usize = 253;
pub const ATT_MTU: usize = MTU + 3;

pub const DEVICE_NAME: &str = "Passport Prime";
pub const SHORT_NAME: &str = "Prime";

pub const SERVICES_LIST: [[u8; 16]; 1] = [NUS_UUID.to_le_bytes()];

// Buffer max bt packets
pub const BT_BUFF_MAX_PKT_LEN: usize = 256;
pub const BT_MAX_NUM_PKT: usize = 4;
// NB! MAX_IRQ depends on chip used, for example: nRF52840 has 48 IRQs, nRF52832 has 38.
pub const MAX_IRQ: u16 = 25;
