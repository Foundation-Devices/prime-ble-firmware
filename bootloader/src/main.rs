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
use host_protocol::{State, TrustLevel};
use panic_probe as _;

use consts::{FLASH_PAGE, FW_CHUNK_SIZE, SEALED_SECRET, SEALED_WIPED, SEAL_IDX};
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
use embassy_nrf::{
    peripherals::SPI0,
    spis::{self, Spis},
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
use postcard::{from_bytes, to_slice};
use sha2::Sha256 as ShaChallenge;
use verify::{get_fw_image_slice, read_version_and_build_date, verify_fw_image, write_secret};

// Global mutex for hardware RNG access
static RNG_HW: CriticalSectionMutex<RefCell<Option<Rng<'_, RNG>>>> = Mutex::new(RefCell::new(None));

// Bind hardware interrupts to their handlers
bind_interrupts!(struct Irqs {
    SPIM0_SPIS0_SPI0 => spis::InterruptHandler<SPI0>;
    RNG => rng::InterruptHandler<RNG>;
    UARTE0_UART0 => uarte::InterruptHandler<UARTE0>;
});

/// State of an in-progress firmware update.
///
/// `Some(n)` means an update session is open (post-erase, pre-finalization) and
/// the next expected packet index is `n`. `None` means no session is open and
/// writes are rejected until the next EraseFirmware. The session ends after a
/// short final block (end of image) or a BootFirmware.
type UpdateSession = Option<u32>;

/// Writes one firmware update chunk at the idx-keyed flash position.
///
/// Position is derived as `BASE_APP_ADDR + idx * FW_CHUNK_SIZE`. The host must
/// send idx in order and re-send the same idx on retry until acked.
/// Non-final blocks must be exactly `FW_CHUNK_SIZE` bytes; a shorter block ends
/// the image and closes the session until the next EraseFirmware.
fn write_firmware_block(idx: usize, data: &[u8], session: &mut UpdateSession, flash: &mut Nvmc<'_>) -> Bootloader<'static> {
    let Some(expected) = *session else {
        return Bootloader::FirmwareOutOfBounds { block_idx: idx };
    };
    if idx as u32 != expected {
        return Bootloader::FirmwareOutOfBounds { block_idx: idx };
    }
    if data.is_empty() || data.len() > FW_CHUNK_SIZE {
        return Bootloader::FirmwareOutOfBounds { block_idx: idx };
    }
    let is_final = data.len() < FW_CHUNK_SIZE;
    let Some(cursor) = (idx as u32)
        .checked_mul(FW_CHUNK_SIZE as u32)
        .and_then(|off| off.checked_add(BASE_APP_ADDR))
    else {
        return Bootloader::FirmwareOutOfBounds { block_idx: idx };
    };
    let Some(end) = cursor.checked_add(data.len() as u32) else {
        return Bootloader::FirmwareOutOfBounds { block_idx: idx };
    };
    if end > BASE_BOOTLOADER_ADDR {
        return Bootloader::FirmwareOutOfBounds { block_idx: idx };
    }
    match flash.write(cursor, data) {
        Ok(()) => {
            *session = if is_final { None } else { Some(expected.saturating_add(1)) };
            let crc = Crc::<u32>::new(&CRC_32_ISCSI).checksum(data);
            Bootloader::AckWithIdxCrc { block_idx: idx, crc }
        }
        Err(_) => Bootloader::NackWithIdx { block_idx: idx },
    }
}

/// Sends a message over SPI using postcard encoding
#[inline(never)]
fn ack_msg_send(message: HostProtocolMessage, spi: &mut Spis<SPI0>) {
    let mut buf = [0_u8; MAX_MSG_SIZE];
    let Ok(resp) = to_slice(&message, &mut buf[2..]) else {
        error!("Failed to serialize response");
        return;
    };
    let resp_len = resp.len();
    buf[..2].copy_from_slice(&u16::to_be_bytes(resp_len as u16));
    let _ = spi.blocking_write_from_ram(&buf[..resp_len + 2]);
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
    let _irq_out_pin = Output::new(p.P0_20, Level::High, OutputDrive::Standard);

    // Initialize flash controller
    let mut flash = Nvmc::new(p.NVMC);

    // Check if secrets are sealed
    let mut seal = unsafe { &*nrf52805_pac::UICR::ptr() }.customer[SEAL_IDX].read().customer().bits();

    let mut update_session: UpdateSession = None;

    let mut raw_buf = [0u8; 512];
    let mut firmware_version: heapless::String<20>;

    // Main command processing loop
    loop {
        let n = spi.read(&mut raw_buf).await;

        if let Ok(n) = n {
            if n == 0 {
                continue;
            }

            let buf = &mut raw_buf[..n];

            if let Some(resp) = match from_bytes(buf) {
                Ok(req) => match req {
                    HostProtocolMessage::Bootloader(boot_msg) => match boot_msg {
                        // Handle firmware erase command
                        Bootloader::EraseFirmware => {
                            trace!("Erase firmware");
                            let start = BASE_APP_ADDR / FLASH_PAGE * FLASH_PAGE;
                            debug!("start: 0x{:08X}", start);
                            update_session = None;
                            if start == BASE_APP_ADDR {
                                if flash.erase(BASE_APP_ADDR, BASE_BOOTLOADER_ADDR).is_ok() {
                                    update_session = Some(0);
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
                                } else if flash.erase(start, BASE_BOOTLOADER_ADDR).is_ok() {
                                    if flash.write(start, &saved).is_err() {
                                        error!("write error");
                                        Some(HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmwareWrite))
                                    } else {
                                        update_session = Some(0);
                                        Some(HostProtocolMessage::Bootloader(Bootloader::AckEraseFirmware))
                                    }
                                } else {
                                    error!("erase error");
                                    Some(HostProtocolMessage::Bootloader(Bootloader::NackEraseFirmware))
                                }
                            }
                        }
                        // Handle firmware block write
                        Bootloader::WriteFirmwareBlock {
                            block_idx: idx,
                            block_data: data,
                        } => Some(HostProtocolMessage::Bootloader(write_firmware_block(
                            idx,
                            data,
                            &mut update_session,
                            &mut flash,
                        ))),
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
                            let result = if seal == SEALED_SECRET || seal == SEALED_WIPED {
                                SecretSaveResponse::NotAllowed
                            } else if secret.iter().all(|word| *word == 0) || secret.iter().all(|word| *word == u32::MAX) {
                                SecretSaveResponse::Error
                            } else {
                                unsafe {
                                    match write_secret(secret, SEALED_SECRET) {
                                        true => {
                                            info!("Challenge secret is saved");
                                            seal = SEALED_SECRET;
                                            SecretSaveResponse::Sealed
                                        }
                                        false => SecretSaveResponse::Error,
                                    }
                                }
                            };
                            Some(HostProtocolMessage::Bootloader(Bootloader::AckChallengeSet { result }))
                        }
                        // Handle request to boot into firmware
                        Bootloader::BootFirmware { trust } => {
                            // Close the update session; a re-entry into the update path must re-erase.
                            update_session = None;
                            match verify_fw_image(trust) {
                                Some((VerificationResult::Valid, VerificationResult::Valid, hash)) => {
                                    info!("fw is valid");
                                    // Clear the authentication secret if we are possibly booting a dev firmware
                                    if seal == SEALED_SECRET && trust == TrustLevel::Developer {
                                        unsafe { write_secret([0; 8], SEALED_WIPED) };
                                    }
                                    let msg = HostProtocolMessage::Bootloader(Bootloader::AckVerifyFirmware { result: true, hash });
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
                                Some((_, _, hash)) => {
                                    info!("fw is invalid");
                                    Some(HostProtocolMessage::Bootloader(Bootloader::AckVerifyFirmware {
                                        result: false,
                                        hash,
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
                        Some(if seal == SEALED_SECRET {
                            type HmacSha256 = Hmac<ShaChallenge>;
                            let secret_as_slice =
                                unsafe { core::slice::from_raw_parts(UICR_SECRET_START as *const u8, UICR_SECRET_SIZE as usize) };
                            // debug!("slice {:02X}", secret_as_slice);
                            if let Ok(mut mac) = HmacSha256::new_from_slice(secret_as_slice) {
                                mac.update(&nonce.to_be_bytes());
                                let result = mac.finalize().into_bytes();
                                // debug!("{=[u8;32]:#X}", result.into());
                                HostProtocolMessage::ChallengeResult { result: result.into() }
                            } else {
                                HostProtocolMessage::ChallengeResult { result: [0xFF; 32] }
                            }
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
