// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
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

use consts::{FLASH_PAGE, SEALED_SECRET, SEAL_IDX};
use consts_global::{BASE_APP_ADDR, BASE_BOOTLOADER_ADDR, SIGNATURE_HEADER_SIZE, UICR_SECRET_SIZE, UICR_SECRET_START};
use core::cell::RefCell;
use cosign2::VerificationResult;
use crc::{Crc, CRC_32_ISCSI};
use defmt::{debug, error, info, trace};
use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts,
    gpio::{Level, Output, OutputDrive},
    nvmc::Nvmc,
    peripherals::RNG,
    rng::{self, Rng},
};
#[cfg(feature = "hw-rev-d")]
use embassy_nrf::{
    peripherals::SPI0,
    spis::{self, Spis},
    Peripheral,
};
use embassy_nrf::{
    peripherals::UARTE0,
    uarte::{self},
};
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use embassy_sync::blocking_mutex::Mutex;
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use hmac::{Hmac, Mac};
use host_protocol::MAX_MSG_SIZE;
use host_protocol::{Bootloader, SecretSaveResponse};
use host_protocol::{HostProtocolMessage, PostcardError};
use jump_app::jump_to_app;
#[allow(unused_imports)]
use nrf_softdevice::Softdevice;
#[cfg(not(feature = "hw-rev-d"))]
use postcard::{
    accumulator::{CobsAccumulator, FeedResult},
    to_slice_cobs,
};
#[cfg(feature = "hw-rev-d")]
use postcard::{from_bytes, to_slice};
use serde::{Deserialize, Serialize};
use sha2::Sha256 as ShaChallenge;
use verify::{get_fw_image_slice, read_version_and_build_date, verify_fw_image, write_secret};

// Global mutex for hardware RNG access
static RNG_HW: CriticalSectionMutex<RefCell<Option<Rng<'_, RNG>>>> = Mutex::new(RefCell::new(None));

// Global static pin for IRQ output
static mut IRQ_OUT_PIN: Option<Output<'static>> = None;

// Bind hardware interrupts to their handlers
#[cfg(not(feature = "hw-rev-d"))]
bind_interrupts!(struct Irqs {
    UARTE0_UART0 => uarte::InterruptHandler<UARTE0>;
    RNG => rng::InterruptHandler<RNG>;
});
#[cfg(feature = "hw-rev-d")]
bind_interrupts!(struct Irqs {
    SPIM0_SPIS0_SPI0 => spis::InterruptHandler<SPI0>;
    RNG => rng::InterruptHandler<RNG>;
    UARTE0_UART0 => uarte::InterruptHandler<UARTE0>;
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
impl BootState {
    fn reset(&mut self) {
        self.offset = 0;
        self.actual_sector = 0;
        self.actual_pkt_idx = 0;
    }
}

/// Signals the MPU by generating a falling edge pulse on the IRQ line
///
/// Pulse sequence: HIGH -> LOW -> HIGH
/// Used to notify MPU of important events or available data
///
/// Safety: Accesses static IRQ_OUT_PIN which is only modified during init
fn assert_out_irq() {
    unsafe {
        if let Some(pin) = IRQ_OUT_PIN.as_mut() {
            // Generate falling edge pulse using the static pin
            pin.set_low();
            pin.set_high();
        }
    }
}

/// Sends a message over UART using postcard with COBS encoding
#[inline(never)]
#[cfg(not(feature = "hw-rev-d"))]
fn ack_msg_send(message: HostProtocolMessage, tx: &mut uarte::UarteTx<UARTE0>) {
    let mut buf_cobs = [0_u8; MAX_MSG_SIZE];
    let cobs_ack = to_slice_cobs(&message, &mut buf_cobs).unwrap();
    let _ = tx.blocking_write(cobs_ack);
    assert_out_irq();
}

/// Sends a message over SPI using postcard encoding
#[inline(never)]
#[cfg(feature = "hw-rev-d")]
fn ack_msg_send(message: HostProtocolMessage, spi: &mut Spis<SPI0>) {
    let mut buf = [0_u8; MAX_MSG_SIZE];
    let Ok(resp) = to_slice(&message, &mut buf) else {
        error!("Failed to serialize response");
        return;
    };
    let resp_len = u16::to_be_bytes(resp.len() as u16);
    assert_out_irq();
    let _ = spi.blocking_write_from_ram(&resp_len);
    let _ = spi.blocking_write_from_ram(&resp);
}

#[cfg(not(feature = "debug"))]
/// Configures flash memory protection for bootloader and MBR regions
///
/// Uses Nordic's BPROT peripheral to prevent modification of critical code regions
/// Activate APP Protection on nrf MCU
/// https://infocenter.nordicsemi.com/topic/ps_nrf52805/uicr.html?cp=5_6_0_3_4_0_5#register.APPROTECT
/// We should check for this part code to check version
/// https://infocenter.nordicsemi.com/pdf/in_153_v1.0.pdf?cp=18_4
fn flash_protect_mbr_bootloader() {
    // Protect Nordic MBR area
    unsafe { &*nrf52805_pac::BPROT::ptr() }.config0.write(|w| {
        w.region0().enabled(); //0x00000-0x01000
        w
    });
    let bits_0 = unsafe { &*nrf52805_pac::BPROT::ptr() }.config0.read().bits();
    debug!("CONFIG0_BITS : {}", bits_0);

    // Protect bootloader area (0x27000-0x30000)
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
        w
    });
    let bits_1 = unsafe { &*nrf52805_pac::BPROT::ptr() }.config1.read().bits();
    debug!("CONFIG1_BITS : {}", bits_1);

    // Enable protection even in debug mode
    unsafe { &*nrf52805_pac::BPROT::ptr() }
        .disableindebug
        .write(|w| unsafe { w.bits(0x00) });
    let disabledebug = unsafe { &*nrf52805_pac::BPROT::ptr() }.disableindebug.read().bits();
    debug!("DISABLE : {}", disabledebug);
}

#[cfg(not(feature = "debug"))]
/// Configures flash memory protection for SoftDevice and Applciation regions
///
/// Uses Nordic's BPROT peripheral to prevent modification of critical code regions
/// Activate APP Protection on nrf MCU
/// https://infocenter.nordicsemi.com/topic/ps_nrf52805/uicr.html?cp=5_6_0_3_4_0_5#register.APPROTECT
/// We should check for this part code to check version
/// https://infocenter.nordicsemi.com/pdf/in_153_v1.0.pdf?cp=18_4
fn flash_protect_sd_application() {
    // Protect Nordic SD area and application area
    unsafe { &*nrf52805_pac::BPROT::ptr() }.config0.write(|w| {
        w.region1().enabled(); //0x01000-0x02000
        w.region2().enabled(); //0x02000-0x03000
        w.region3().enabled(); //0x03000-0x04000
        w.region4().enabled(); //0x04000-0x05000
        w.region5().enabled(); //0x05000-0x06000
        w.region6().enabled(); //0x06000-0x07000
        w.region7().enabled(); //0x07000-0x08000
        w.region8().enabled(); //0x08000-0x09000
        w.region9().enabled(); //0x09000-0x0A000
        w.region10().enabled(); //0x0A000-0x0B000
        w.region11().enabled(); //0x0B000-0x0C000
        w.region12().enabled(); //0x0C000-0x0D000
        w.region13().enabled(); //0x0D000-0x0E000
        w.region14().enabled(); //0x0E000-0x0F000
        w.region15().enabled(); //0x0F000-0x10000
        w.region16().enabled(); //0x10000-0x11000
        w.region17().enabled(); //0x11000-0x12000
        w.region18().enabled(); //0x12000-0x13000
        w.region19().enabled(); //0x13000-0x14000
        w.region20().enabled(); //0x14000-0x15000
        w.region21().enabled(); //0x15000-0x16000
        w.region22().enabled(); //0x16000-0x17000
        w.region23().enabled(); //0x17000-0x18000
        w.region24().enabled(); //0x18000-0x19000
        w.region25().enabled(); //0x19000-0x1A000
        w.region26().enabled(); //0x1A000-0x1B000
        w.region27().enabled(); //0x1B000-0x1C000
        w.region28().enabled(); //0x1C000-0x1D000
        w.region29().enabled(); //0x1D000-0x1E000
        w.region30().enabled(); //0x1E000-0x1F000
        w.region31().enabled(); //0x1F000-0x20000
        w
    });
    let bits_0 = unsafe { &*nrf52805_pac::BPROT::ptr() }.config0.read().bits();
    debug!("CONFIG0_BITS : {}", bits_0);

    // Protect Nordic SD area and application area
    unsafe { &*nrf52805_pac::BPROT::ptr() }.config1.write(|w| {
        w.region32().enabled(); //0x20000-0x21000
        w.region33().enabled(); //0x21000-0x22000
        w.region34().enabled(); //0x22000-0x23000
        w.region35().enabled(); //0x23000-0x24000
        w.region36().enabled(); //0x24000-0x25000
        w.region37().enabled(); //0x25000-0x26000
        w.region38().enabled(); //0x26000-0x27000
        w
    });
    let bits_1 = unsafe { &*nrf52805_pac::BPROT::ptr() }.config1.read().bits();
    debug!("CONFIG1_BITS : {}", bits_1);
}

/// Main bootloader entry point
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    #[cfg(not(feature = "debug"))]
    flash_protect_mbr_bootloader();

    let p = embassy_nrf::init(Default::default());

    #[cfg(not(feature = "hw-rev-d"))]
    let (mut tx, mut rx) = {
        // Configure UART
        let mut config_uart = uarte::Config::default();
        config_uart.parity = uarte::Parity::EXCLUDED;

        #[cfg(not(feature = "debug"))]
        let (rxd, txd, baud_rate) = (p.P0_14, p.P0_12, uarte::Baudrate::BAUD460800);

        #[cfg(feature = "debug")]
        let (rxd, txd, baud_rate) = (p.P0_16, p.P0_18, uarte::Baudrate::BAUD460800);

        config_uart.baudrate = baud_rate;

        let uart = uarte::Uarte::new(p.UARTE0, Irqs, rxd, txd, config_uart);
        uart.split_with_idle(p.TIMER0, p.PPI_CH0, p.PPI_CH1)
    };

    // Send a wake-up sequence to the MPU (SFT-5196 workaround)
    #[cfg(feature = "hw-rev-d")]
    {
        let tx = unsafe { p.P0_16.clone_unchecked() };
        let mut config_uart = uarte::Config::default();
        config_uart.parity = uarte::Parity::EXCLUDED;
        config_uart.baudrate = uarte::Baudrate::BAUD2400;

        let mut uart = uarte::UarteTx::new(p.UARTE0, Irqs, tx, config_uart);
        uart.write(&[0xAA]).await.unwrap();
    }

    #[cfg(feature = "hw-rev-d")]
    let mut spi = {
        // Configure SPI
        let mut config_spi = spis::Config::default();
        config_spi.orc = 0x69; // to detect padding
        Spis::new(p.SPI0, Irqs, p.P0_18, p.P0_16, p.P0_14, p.P0_12, config_spi)
    };

    // Initialize hardware RNG
    let rng = Rng::new(p.RNG, Irqs);
    {
        RNG_HW.lock(|f| f.borrow_mut().replace(rng));
    }

    // Initialize IRQ_OUT pin
    unsafe {
        IRQ_OUT_PIN = Some(Output::new(p.P0_20, Level::High, OutputDrive::Standard));
    }

    // Initialize flash controller
    let mut flash = Nvmc::new(p.NVMC);

    // Check if secrets are sealed
    let seal = unsafe { &*nrf52805_pac::UICR::ptr() }.customer[SEAL_IDX].read().customer().bits();

    // Send startup message (UART console only)
    // Removed to save Flash Space
    // #[cfg(all(feature = "debug", not(feature = "hw-rev-d")))]
    // {
    //     let mut buf = [0; 10];
    //     buf.copy_from_slice(b"Bootloader");
    //     let _ = tx.write(&buf).await;
    // }
    let mut boot_status: BootState = Default::default();

    let mut raw_buf = [0u8; 512];
    #[cfg(not(feature = "hw-rev-d"))]
    let mut cobs_buf: CobsAccumulator<MAX_MSG_SIZE> = CobsAccumulator::new();
    let mut firmware_version: heapless::String<20>;

    // Main command processing loop
    loop {
        #[cfg(not(feature = "hw-rev-d"))]
        let n = rx.read_until_idle(&mut raw_buf).await;
        #[cfg(feature = "hw-rev-d")]
        let n = spi.read(&mut raw_buf).await;

        if let Ok(n) = n {
            if n == 0 {
                continue;
            }

            let buf = &mut raw_buf[..n];
            #[cfg(not(feature = "hw-rev-d"))]
            let mut window: &[u8] = buf;
            #[cfg(not(feature = "hw-rev-d"))]
            let mut resp;

            #[cfg(not(feature = "hw-rev-d"))]
            'cobs: while !window.is_empty() {
                (window, resp) = match cobs_buf.feed_ref::<HostProtocolMessage>(window) {
                    FeedResult::Consumed => {
                        trace!("consumed");
                        break 'cobs;
                    }
                    FeedResult::OverFull(new_wind) => {
                        trace!("overfull");
                        (new_wind, Some(HostProtocolMessage::PostcardError(PostcardError::OverFull)))
                    }
                    FeedResult::DeserError(new_wind) => {
                        trace!("DeserError");
                        (new_wind, Some(HostProtocolMessage::PostcardError(PostcardError::Deser)))
                    }
                    FeedResult::Success { data: req, remaining } => {
                        trace!("Success");
                        debug!("Remaining {} bytes", remaining.len());
                        (
                            remaining,
                            match req {
                                HostProtocolMessage::Bootloader(boot_msg) => match boot_msg {
                                    // Handle firmware erase command
                                    Bootloader::EraseFirmware => {
                                        trace!("Erase firmware");
                                        let start = BASE_APP_ADDR / FLASH_PAGE * FLASH_PAGE;
                                        debug!("start: 0x{:08X}", start);
                                        if start == BASE_APP_ADDR {
                                            if flash.erase(BASE_APP_ADDR, BASE_BOOTLOADER_ADDR).is_ok() {
                                                boot_status.reset();
                                                Some(HostProtocolMessage::Bootloader(Bootloader::AckEraseFirmware))
                                            } else {
                                                error!("erase error");
                                                Some(HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmware))
                                            }
                                        } else {
                                            let mut saved = [0; (BASE_APP_ADDR % FLASH_PAGE) as usize];
                                            if flash.read(start, &mut saved).is_err() {
                                                error!("read error");
                                                Some(HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmwareRead))
                                            } else {
                                                if flash.erase(start, BASE_BOOTLOADER_ADDR).is_ok() {
                                                    boot_status.reset();
                                                    if flash.write(start, &saved).is_err() {
                                                        error!("write error");
                                                        Some(HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmwareWrite))
                                                    } else {
                                                        Some(HostProtocolMessage::Bootloader(Bootloader::AckEraseFirmware))
                                                    }
                                                } else {
                                                    error!("erase error");
                                                    Some(HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmware))
                                                }
                                            }
                                        }
                                    }
                                    // Handle firmware block write
                                    Bootloader::WriteFirmwareBlock {
                                        block_idx: idx,
                                        block_data: data,
                                    } => {
                                        // Calculate target flash address
                                        let cursor = BASE_APP_ADDR + boot_status.offset;
                                        Some(
                                            // Validate write is within application area
                                            HostProtocolMessage::Bootloader(if (BASE_APP_ADDR..=BASE_BOOTLOADER_ADDR).contains(&cursor) {
                                                match flash.write(cursor, data) {
                                                    Ok(()) => {
                                                        boot_status.offset += data.len() as u32;
                                                        // Update status and sector tracking
                                                        boot_status.actual_sector =
                                                            BASE_APP_ADDR + (boot_status.offset / FLASH_PAGE) * FLASH_PAGE;
                                                        debug!("Updating flash page starting at addr: {:02X}", boot_status.actual_sector);
                                                        debug!(
                                                            "offset : {:02X}",
                                                            boot_status.actual_sector + boot_status.offset % FLASH_PAGE
                                                        );

                                                        // Calculate CRC of written data
                                                        let crc = Crc::<u32>::new(&CRC_32_ISCSI);
                                                        let crc_pkt = crc.checksum(data);

                                                        boot_status.actual_pkt_idx = idx as u32;

                                                        // Send success acknowledgement with CRC
                                                        Bootloader::AckWithIdxCrc {
                                                            block_idx: idx,
                                                            crc: crc_pkt,
                                                        }
                                                    }
                                                    Err(_) => Bootloader::NackWithIdx { block_idx: idx },
                                                }
                                            } else {
                                                Bootloader::FirmwareOutOfBounds { block_idx: idx }
                                            }),
                                        )
                                    }
                                    // Get firmware version from header
                                    Bootloader::FirmwareVersion => {
                                        let image = get_fw_image_slice(BASE_APP_ADDR, SIGNATURE_HEADER_SIZE);
                                        if let Some((version, _build_date)) = read_version_and_build_date(image, false) {
                                            firmware_version = version;
                                            Some(HostProtocolMessage::Bootloader(Bootloader::AckFirmwareVersion {
                                                version: &firmware_version,
                                            }))
                                        } else {
                                            Some(HostProtocolMessage::Bootloader(Bootloader::NoCosignHeader))
                                        }
                                    }
                                    // Get bootloader version
                                    Bootloader::BootloaderVersion => {
                                        let version = env!("CARGO_PKG_VERSION");
                                        Some(HostProtocolMessage::Bootloader(Bootloader::AckBootloaderVersion { version }))
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
                                        Some(HostProtocolMessage::Bootloader(Bootloader::AckChallengeSet { result }))
                                    }
                                    // Handle request to boot into firmware
                                    Bootloader::BootFirmware => {
                                        // Double check firmware validity before jumping
                                        match verify_fw_image() {
                                            Some((VerificationResult::Valid, hash)) => {
                                                info!("fw is valid");
                                                let msg = HostProtocolMessage::Bootloader(Bootloader::AckVerifyFirmware {
                                                    result: true,
                                                    hash: hash.sha,
                                                });
                                                #[cfg(not(feature = "debug"))]
                                                flash_protect_sd_application();
                                                // immedate send response before jumping
                                                ack_msg_send(msg, &mut tx);
                                                // Clean up UART resources before jumping
                                                drop(tx);
                                                drop(rx);
                                                // Jump to application code if firmware is valid
                                                unsafe {
                                                    jump_to_app();
                                                }
                                            }
                                            Some((VerificationResult::Invalid, hash)) => {
                                                info!("fw is invalid");
                                                Some(HostProtocolMessage::Bootloader(Bootloader::AckVerifyFirmware {
                                                    result: false,
                                                    hash: hash.sha,
                                                }))
                                            }
                                            None => Some(HostProtocolMessage::Bootloader(Bootloader::NoCosignHeader)),
                                        }
                                    }
                                    _ => None,
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
                                    // debug!("slice {:02X}", secret_as_slice);
                                    Some(if let Ok(mut mac) = HmacSha256::new_from_slice(secret_as_slice) {
                                        mac.update(&nonce.to_be_bytes());
                                        let result = mac.finalize().into_bytes();
                                        // debug!("{=[u8;32]:#X}", result.into());
                                        HostProtocolMessage::ChallengeResult { result: result.into() }
                                    } else {
                                        HostProtocolMessage::ChallengeResult { result: [0xFF; 32] }
                                    })
                                }
                                // Report bootloader state
                                HostProtocolMessage::GetState => Some(HostProtocolMessage::AckState(State::FirmwareUpgrade)),
                                _ => Some(HostProtocolMessage::InappropriateMessage(State::FirmwareUpgrade)),
                            },
                        )
                    }
                };
                if let Some(resp) = resp {
                    ack_msg_send(resp, &mut tx);
                }
            }

            #[cfg(feature = "hw-rev-d")]
            if let Some(resp) = match from_bytes(buf) {
                Ok(req) => match req {
                    HostProtocolMessage::Bootloader(boot_msg) => match boot_msg {
                        // Handle firmware erase command
                        Bootloader::EraseFirmware => {
                            trace!("Erase firmware");
                            let start = BASE_APP_ADDR / FLASH_PAGE * FLASH_PAGE;
                            debug!("start: 0x{:08X}", start);
                            if start == BASE_APP_ADDR {
                                if flash.erase(BASE_APP_ADDR, BASE_BOOTLOADER_ADDR).is_ok() {
                                    boot_status.reset();
                                    Some(HostProtocolMessage::Bootloader(Bootloader::AckEraseFirmware))
                                } else {
                                    error!("erase error");
                                    Some(HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmware))
                                }
                            } else {
                                let mut saved = [0; (BASE_APP_ADDR % FLASH_PAGE) as usize];
                                if flash.read(start, &mut saved).is_err() {
                                    error!("read error");
                                    Some(HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmwareRead))
                                } else {
                                    if flash.erase(start, BASE_BOOTLOADER_ADDR).is_ok() {
                                        boot_status.reset();
                                        if flash.write(start, &saved).is_err() {
                                            error!("write error");
                                            Some(HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmwareWrite))
                                        } else {
                                            Some(HostProtocolMessage::Bootloader(Bootloader::AckEraseFirmware))
                                        }
                                    } else {
                                        error!("erase error");
                                        Some(HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmware))
                                    }
                                }
                            }
                        }
                        // Handle firmware block write
                        Bootloader::WriteFirmwareBlock {
                            block_idx: idx,
                            block_data: data,
                        } => {
                            // Calculate target flash address
                            let cursor = BASE_APP_ADDR + boot_status.offset;
                            Some(
                                // Validate write is within application area
                                HostProtocolMessage::Bootloader(if (BASE_APP_ADDR..=BASE_BOOTLOADER_ADDR).contains(&cursor) {
                                    match flash.write(cursor, data) {
                                        Ok(()) => {
                                            boot_status.offset += data.len() as u32;
                                            // Update status and sector tracking
                                            boot_status.actual_sector = BASE_APP_ADDR + (boot_status.offset / FLASH_PAGE) * FLASH_PAGE;
                                            debug!("Updating flash page starting at addr: {:02X}", boot_status.actual_sector);
                                            debug!("offset : {:02X}", boot_status.actual_sector + boot_status.offset % FLASH_PAGE);

                                            // Calculate CRC of written data
                                            let crc = Crc::<u32>::new(&CRC_32_ISCSI);
                                            let crc_pkt = crc.checksum(data);

                                            boot_status.actual_pkt_idx = idx as u32;

                                            // Send success acknowledgement with CRC
                                            Bootloader::AckWithIdxCrc {
                                                block_idx: idx,
                                                crc: crc_pkt,
                                            }
                                        }
                                        Err(_) => Bootloader::NackWithIdx { block_idx: idx },
                                    }
                                } else {
                                    Bootloader::FirmwareOutOfBounds { block_idx: idx }
                                }),
                            )
                        }
                        // Get firmware version from header
                        Bootloader::FirmwareVersion => {
                            let image = get_fw_image_slice(BASE_APP_ADDR, SIGNATURE_HEADER_SIZE);
                            if let Some((version, _build_date)) = read_version_and_build_date(image, false) {
                                firmware_version = version;
                                Some(HostProtocolMessage::Bootloader(Bootloader::AckFirmwareVersion {
                                    version: &firmware_version,
                                }))
                            } else {
                                Some(HostProtocolMessage::Bootloader(Bootloader::NoCosignHeader))
                            }
                        }
                        // Get bootloader version
                        Bootloader::BootloaderVersion => {
                            let version = env!("CARGO_PKG_VERSION");
                            Some(HostProtocolMessage::Bootloader(Bootloader::AckBootloaderVersion { version }))
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
                            Some(HostProtocolMessage::Bootloader(Bootloader::AckChallengeSet { result }))
                        }
                        // Handle request to boot into firmware
                        Bootloader::BootFirmware => {
                            // Double check firmware validity before jumping
                            match verify_fw_image() {
                                Some((VerificationResult::Valid, hash)) => {
                                    info!("fw is valid");
                                    let msg = HostProtocolMessage::Bootloader(Bootloader::AckVerifyFirmware {
                                        result: true,
                                        hash: hash.sha,
                                    });
                                    #[cfg(not(feature = "debug"))]
                                    flash_protect_sd_application();
                                    // immedate send response before jumping
                                    ack_msg_send(msg, &mut spi);
                                    // Clean up SPI resources before jumping
                                    drop(spi);
                                    // Jump to application code if firmware is valid
                                    unsafe {
                                        jump_to_app();
                                    }
                                }
                                Some((VerificationResult::Invalid, hash)) => {
                                    info!("fw is invalid");
                                    Some(HostProtocolMessage::Bootloader(Bootloader::AckVerifyFirmware {
                                        result: false,
                                        hash: hash.sha,
                                    }))
                                }
                                None => Some(HostProtocolMessage::Bootloader(Bootloader::NoCosignHeader)),
                            }
                        }
                        _ => None,
                    },
                    // Handle reset command
                    HostProtocolMessage::Reset => {
                        drop(spi);
                        cortex_m::peripheral::SCB::sys_reset();
                    }
                    // Handle challenge-response authentication
                    HostProtocolMessage::ChallengeRequest { nonce } => {
                        type HmacSha256 = Hmac<ShaChallenge>;
                        let secret_as_slice =
                            unsafe { core::slice::from_raw_parts(UICR_SECRET_START as *const u8, UICR_SECRET_SIZE as usize) };
                        // debug!("slice {:02X}", secret_as_slice);
                        Some(if let Ok(mut mac) = HmacSha256::new_from_slice(secret_as_slice) {
                            mac.update(&nonce.to_be_bytes());
                            let result = mac.finalize().into_bytes();
                            // debug!("{=[u8;32]:#X}", result.into());
                            HostProtocolMessage::ChallengeResult { result: result.into() }
                        } else {
                            HostProtocolMessage::ChallengeResult { result: [0xFF; 32] }
                        })
                    }
                    // Report bootloader state
                    HostProtocolMessage::GetState => Some(HostProtocolMessage::AckState(State::FirmwareUpgrade)),
                    _ => Some(HostProtocolMessage::InappropriateMessage(State::FirmwareUpgrade)),
                },
                Err(_) => Some(HostProtocolMessage::PostcardError(PostcardError::Deser)),
            } {
                ack_msg_send(resp, &mut spi);
            }
            embassy_time::Timer::after_millis(1).await;
        }
    }
}
