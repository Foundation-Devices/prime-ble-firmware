use crate::nus::NUS_UUID;

/// Maximum Transfer Unit (MTU) size for BLE communication
pub const MTU: usize = 256;

/// ATT MTU size includes 3 bytes of overhead on top of base MTU
pub const ATT_MTU: usize = MTU + 3;

/// Full device name advertised over BLE
pub const DEVICE_NAME: &str = "Passport Prime";

/// Short device name used in limited advertising data
pub const SHORT_NAME: &str = "Prime";

/// List of BLE service UUIDs supported by this device.
/// Currently only includes the Nordic UART Service (NUS).
pub const SERVICES_LIST: [[u8; 16]; 1] = [NUS_UUID.to_le_bytes()];

/// Maximum number of BLE packets that can be buffered.
/// This limits memory usage while ensuring reliable data transfer.
pub const BT_MAX_NUM_PKT: usize = 4;

/// Starting address in UICR (User Information Configuration Registers)
/// where the device secret is stored. UICR is non-volatile memory.
pub const UICR_SECRET_START: u32 = 0x10001080;

/// Size in bytes of the secret stored in UICR.
/// 0x10 = 16 bytes = 128 bits
pub const UICR_SECRET_SIZE: u32 = 0x10;
