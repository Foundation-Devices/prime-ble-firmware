// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bootloader implementation for Foundation Devices hardware
//!
//! This bootloader provides:
//! - Firmware update capabilities over UART
//! - Firmware verification using cosign signatures
//! - Secret storage and challenge-response authentication
//! - Flash memory protection
//! - Secure boot process

#![no_std]
#![no_main]
mod consts;
mod jump_app;
mod verify;

use defmt_rtt as _;
use embassy_nrf::{self as _};
use host_protocol::State;
use panic_probe as _;

use consts::*;
use core::cell::RefCell;
use cosign2::{Header, VerificationResult};
use crc::{Crc, CRC_32_ISCSI};
use defmt::info;
use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts,
    gpio::{Level, Output, OutputDrive},
    nvmc::Nvmc,
    peripherals::{self, RNG, UARTE0},
    rng::{self, Rng},
    uarte::{self, UarteTx},
};
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use embassy_sync::blocking_mutex::Mutex;
use embedded_storage::nor_flash::NorFlash;
use hmac::{Hmac, Mac};
use host_protocol::COBS_MAX_MSG_SIZE;
use host_protocol::{Bootloader, SecretSaveResponse};
use host_protocol::{HostProtocolMessage, PostcardError};
use jump_app::jump_to_app;
#[allow(unused_imports)]
use nrf_softdevice::Softdevice;
use postcard::accumulator::{CobsAccumulator, FeedResult};
use postcard::to_slice_cobs;
use serde::{Deserialize, Serialize};
use sha2::Sha256 as ShaChallenge;
use verify::{check_fw, get_fw_image_slice, write_secret};

// Global mutex for hardware RNG access
static RNG_HW: CriticalSectionMutex<RefCell<Option<Rng<'_, RNG>>>> = Mutex::new(RefCell::new(None));

// Global static pin for IRQ output
static mut IRQ_PIN: Option<Output<'static>> = None;

// Bind hardware interrupts to their handlers
bind_interrupts!(struct Irqs {
    UARTE0_UART0 => uarte::InterruptHandler<peripherals::UARTE0>;
    RNG => rng::InterruptHandler<peripherals::RNG>;
});

/// Tracks the state of firmware updates
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BootState {
    /// Current offset into flash memory
    pub offset: u32,
    /// Current flash sector being written
    pub actual_sector: u32,
    /// Index of the last successfully written packet
    pub actual_pkt_idx: u32,
}
/// Signals the MPU by generating a falling edge pulse on the IRQ line
///
/// Pulse sequence: HIGH -> LOW -> HIGH
/// Used to notify MPU of important events or available data
///
/// Safety: Accesses static IRQ_PIN which is only modified during init
fn assert_out_irq() {
    unsafe {
        if let Some(pin) = IRQ_PIN.as_mut() {
            pin.set_high();
            pin.set_low();
            pin.set_high();
        } // Generate falling edge pulse using the static pin
    }
}

/// Updates a chunk of flash memory with new firmware data
///
/// Validates that the write is within bounds and sends acknowledgement messages
/// back over UART with CRC verification
fn update_chunk<'a>(boot_status: &'a mut BootState, idx: usize, data: &'a [u8], flash: &'a mut Nvmc, tx: &mut UarteTx<UARTE0>) {
    // Calculate target flash address
    let cursor = BASE_APP_ADDR + boot_status.offset;

    // Validate write is within application area
    match cursor {
        (BASE_APP_ADDR..=BASE_BOOTLOADER_APP) => {}
        _ => {
            ack_msg_send(
                HostProtocolMessage::Bootloader(Bootloader::FirmwareOutOfBounds { block_idx: idx }),
                tx,
            );
            return;
        }
    }

    match flash.write(cursor, data) {
        Ok(()) => {
            boot_status.offset += data.len() as u32;
            // Update status and sector tracking
            boot_status.actual_sector = BASE_APP_ADDR + (boot_status.offset / FLASH_PAGE) * FLASH_PAGE;
            info!("Updating flash page starting at addr: {:02X}", boot_status.actual_sector);
            info!("offset : {:02X}", boot_status.actual_sector + boot_status.offset % FLASH_PAGE);

            // Calculate CRC of written data
            let crc = Crc::<u32>::new(&CRC_32_ISCSI);
            let crc_pkt = crc.checksum(data);

            boot_status.actual_pkt_idx = idx as u32;

            // Send success acknowledgement with CRC
            ack_msg_send(
                HostProtocolMessage::Bootloader(Bootloader::AckWithIdxCrc {
                    block_idx: idx,
                    crc: crc_pkt,
                }),
                tx,
            );
        }
        Err(_) => ack_msg_send(HostProtocolMessage::Bootloader(Bootloader::NackWithIdx { block_idx: idx }), tx),
    };
}

/// Sends a message over UART using COBS encoding
#[inline(never)]
pub fn ack_msg_send(message: HostProtocolMessage, tx: &mut UarteTx<UARTE0>) {
    let mut buf_cobs = [0_u8; COBS_MAX_MSG_SIZE];
    let cobs_ack = to_slice_cobs(&message, &mut buf_cobs).unwrap();
    let _ = tx.blocking_write(cobs_ack);
    assert_out_irq();
}

#[cfg(feature = "flash-protect")]
/// Configures flash memory protection for bootloader and MBR regions
///
/// Uses Nordic's BPROT peripheral to prevent modification of critical code regions
fn flash_protect() {
    // Protect Nordic MBR area
    unsafe { &*nrf52805_pac::BPROT::ptr() }.config0.write(|w| w.region0().enabled());
    let bits_0 = unsafe { &*nrf52805_pac::BPROT::ptr() }.config0.read().bits();
    info!("CONFIG0_BITS : {}", bits_0);

    // Protect bootloader area (0x26000-0x30000)
    unsafe { &*nrf52805_pac::BPROT::ptr() }.config1.write(|w| {
        w.region47().enabled(); //0x2F000-0x30000
        w.region46().enabled(); //0x2E000-0x2F000
        w.region45().enabled(); //0x2D000-0x2E000
        w.region44().enabled(); //0x2C000-0x2D000
        w.region43().enabled(); //0x2B000-0x2C000
        w.region42().enabled(); //0x2A000-0x2B000
        w.region41().enabled(); //0x29000-0x2A000
        w.region40().enabled(); //0x28000-0x29000
        w.region39().enabled(); //0x27000-0x28000
        w.region38().enabled(); //0x26000-0x27000
        w
    });
    let bits_1 = unsafe { &*nrf52805_pac::BPROT::ptr() }.config1.read().bits();
    info!("CONFIG1_BITS : {}", bits_1);

    // Enable protection even in debug mode
    unsafe { &*nrf52805_pac::BPROT::ptr() }
        .disableindebug
        .write(|w| unsafe { w.bits(0x00) });
    let disabledebug = unsafe { &*nrf52805_pac::BPROT::ptr() }.disableindebug.read().bits();
    info!("DISABLE : {}", disabledebug);
}

/// Main bootloader entry point
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    #[cfg(feature = "flash-protect")]
    flash_protect();

    let p = embassy_nrf::init(Default::default());

    // Configure UART
    let mut config_uart = uarte::Config::default();
    config_uart.parity = uarte::Parity::EXCLUDED;
    config_uart.baudrate = uarte::Baudrate::BAUD115200;

    #[cfg(feature = "uart-pins-mpu")]
    let (rxd, txd, baud_rate) = (p.P0_14, p.P0_12, uarte::Baudrate::BAUD460800);

    #[cfg(feature = "uart-pins-console")]
    let (rxd, txd, baud_rate) = (p.P0_16, p.P0_18, uarte::Baudrate::BAUD115200);

    config_uart.baudrate = baud_rate;

    let uart = uarte::Uarte::new(p.UARTE0, Irqs, rxd, txd, config_uart);
    let (mut tx, mut rx) = uart.split_with_idle(p.TIMER0, p.PPI_CH0, p.PPI_CH1);

    // Initialize hardware RNG
    let rng = Rng::new(p.RNG, Irqs);
    {
        RNG_HW.lock(|f| f.borrow_mut().replace(rng));
    }

    // Initialize IRQ pin
    unsafe {
        IRQ_PIN = Some(Output::new(p.P0_20, Level::High, OutputDrive::Standard));
    }

    // Initialize flash controller
    let mut flash = Nvmc::new(p.NVMC);

    // Check if secrets are sealed
    let seal = unsafe { &*nrf52805_pac::UICR::ptr() }.customer[SEAL_IDX].read().customer().bits();

    // Send startup message (console only)
    #[cfg(feature = "uart-pins-console")]
    {
        let mut buf = [0; 10];
        buf.copy_from_slice(b"Bootloader");
        let _ = tx.write(&buf).await;
    }
    let mut boot_status: BootState = Default::default();

    // Main command processing loop
    loop {
        let mut raw_buf = [0u8; 512];
        let mut cobs_buf: CobsAccumulator<COBS_MAX_MSG_SIZE> = CobsAccumulator::new();

        // Read and process UART data
        while let Ok(n) = rx.read_until_idle(&mut raw_buf).await {
            if n == 0 {
                break;
            }

            let buf = &raw_buf[..n];
            let mut window: &[u8] = buf;

            'cobs: while !window.is_empty() {
                window = match cobs_buf.feed_ref::<HostProtocolMessage>(window) {
                    FeedResult::Consumed => {
                        info!("consumed");
                        break 'cobs;
                    }
                    FeedResult::OverFull(new_wind) => {
                        info!("overfull");
                        let msg = HostProtocolMessage::PostcardError(PostcardError::OverFull);
                        ack_msg_send(msg, &mut tx);
                        new_wind
                    }
                    FeedResult::DeserError(new_wind) => {
                        info!("DeserError");
                        let msg = HostProtocolMessage::PostcardError(PostcardError::Deser);
                        ack_msg_send(msg, &mut tx);
                        new_wind
                    }
                    FeedResult::Success { data, remaining } => {
                        info!("Remaining {} bytes", remaining.len());

                        match data {
                            HostProtocolMessage::Bluetooth(_) => (), // Pass through to app
                            HostProtocolMessage::Bootloader(boot_msg) => match boot_msg {
                                // Handle firmware erase command
                                Bootloader::EraseFirmware => {
                                    info!("Erase firmware");
                                    if flash.erase(BASE_APP_ADDR, BASE_BOOTLOADER_APP).is_ok() {
                                        ack_msg_send(HostProtocolMessage::Bootloader(Bootloader::AckEraseFirmware), &mut tx);
                                        boot_status = Default::default();
                                    }
                                }
                                // Handle firmware block write
                                Bootloader::WriteFirmwareBlock {
                                    block_idx: idx,
                                    block_data: data,
                                } => {
                                    update_chunk(&mut boot_status, idx, data, &mut flash, &mut tx);
                                }
                                // Get firmware version from header
                                Bootloader::FirmwareVersion => {
                                    let header_slice = get_fw_image_slice(BASE_APP_ADDR, APP_SIZE);
                                    if let Ok(Some(header)) = Header::parse_unverified(header_slice) {
                                        let version = header.version();
                                        ack_msg_send(HostProtocolMessage::Bootloader(Bootloader::AckFirmwareVersion { version }), &mut tx)
                                    } else {
                                        ack_msg_send(HostProtocolMessage::Bootloader(Bootloader::NoCosignHeader), &mut tx);
                                    }
                                }
                                // Get bootloader version
                                Bootloader::BootloaderVersion => {
                                    let version = env!("CARGO_PKG_VERSION");
                                    ack_msg_send(
                                        HostProtocolMessage::Bootloader(Bootloader::AckBootloaderVersion { version }),
                                        &mut tx,
                                    )
                                }
                                // Verify firmware signature
                                Bootloader::VerifyFirmware => {
                                    let image_slice = get_fw_image_slice(BASE_APP_ADDR, APP_SIZE);
                                    match check_fw(image_slice) {
                                        (msg, VerificationResult::Valid) => {
                                            ack_msg_send(msg, &mut tx);
                                            info!("fw is valid");
                                        }
                                        (msg, VerificationResult::Invalid) => {
                                            ack_msg_send(msg, &mut tx);
                                            info!("fw is invalid");
                                        }
                                    }
                                }
                                // Set challenge secret
                                Bootloader::ChallengeSet { secret } => {
                                    let result = if seal == SEALED_SECRET {
                                        SecretSaveResponse::NotAllowed
                                    } else {
                                        unsafe {
                                            match write_secret(secret) {
                                                true => {
                                                    info!("Challenge secret is saved");
                                                    SecretSaveResponse::Sealed
                                                }
                                                false => SecretSaveResponse::Error,
                                            }
                                        }
                                    };
                                    ack_msg_send(HostProtocolMessage::Bootloader(Bootloader::AckChallengeSet { result }), &mut tx);
                                }
                                // Handle request to boot into firmware
                                Bootloader::BootFirmware => {
                                    // Double check firmware validity before jumping
                                    let image_slice = get_fw_image_slice(BASE_APP_ADDR, APP_SIZE);
                                    let (msg, verif_res) = check_fw(image_slice);
                                    ack_msg_send(msg, &mut tx);

                                    if let VerificationResult::Valid = verif_res {
                                        // Clean up UART resources before jumping
                                        drop(tx);
                                        drop(rx);
                                        // Jump to application code if firmware is valid
                                        unsafe {
                                            jump_to_app();
                                        }
                                    }
                                }
                                _ => (),
                            },
                            // Handle reset command
                            HostProtocolMessage::Reset => {
                                drop(tx);
                                drop(rx);
                                cortex_m::peripheral::SCB::sys_reset();
                            }
                            // Handle challenge-response authentication
                            HostProtocolMessage::ChallengeRequest { nonce } => {
                                type HmacSha256 = Hmac<ShaChallenge>;
                                let secret_as_slice =
                                    unsafe { core::slice::from_raw_parts(UICR_SECRET_START as *const u8, UICR_SECRET_SIZE as usize) };
                                info!("slice {:02X}", secret_as_slice);

                                let result = if let Ok(mut mac) = HmacSha256::new_from_slice(secret_as_slice) {
                                    mac.update(&nonce.to_be_bytes());
                                    let result = mac.finalize().into_bytes();
                                    info!("{=[u8;32]:#X}", result.into());
                                    HostProtocolMessage::ChallengeResult { result: result.into() }
                                } else {
                                    HostProtocolMessage::ChallengeResult { result: [0xFF; 32] }
                                };
                                ack_msg_send(result, &mut tx);
                            }
                            // Report bootloader state
                            HostProtocolMessage::GetState => {
                                ack_msg_send(HostProtocolMessage::AckState(State::FirmwareUpgrade), &mut tx);
                            }
                            _ => (),
                        };
                        remaining
                    }
                };
            }
            embassy_time::Timer::after_millis(1).await;
        }
    }
}
