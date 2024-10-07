// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]
#![no_main]
mod consts;
mod jump_app;
mod verify;

use defmt_rtt as _;
use embassy_nrf as _;
use embassy_time::Timer;
use panic_probe as _;

use consts::*;
use host_protocol::COBS_MAX_MSG_SIZE;
use core::cell::RefCell;
use cosign2::Header;
use cosign2::Sha256;
use crc::{Crc, CRC_32_ISCSI};
use defmt::info;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Input, Pull};
use embassy_nrf::nvmc::Nvmc;
use embassy_nrf::peripherals::{self, RNG, UARTE0};
use embassy_nrf::rng;
use embassy_nrf::rng::Rng;
use embassy_nrf::uarte::UarteTx;
use embassy_nrf::{bind_interrupts, uarte};
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use embassy_sync::blocking_mutex::Mutex;
use embedded_storage::nor_flash::NorFlash;
use host_protocol::HostProtocolMessage;
use host_protocol::{Bootloader, SecretSaveResponse};
use jump_app::jump_to_app;
#[allow(unused_imports)]
use nrf_softdevice::Softdevice;
use postcard::accumulator::{CobsAccumulator, FeedResult};
use postcard::to_slice_cobs;
use serde::{Deserialize, Serialize};
use sha2::digest::consts::False;
use verify::{check_fw, get_fw_image_slice, write_secret, Sha256 as sha};

// Mutex for random hw generator to delay in verification
static RNG_HW: CriticalSectionMutex<RefCell<Option<Rng<'_, RNG>>>> = Mutex::new(RefCell::new(None));

bind_interrupts!(struct Irqs {
    UARTE0_UART0 => uarte::InterruptHandler<peripherals::UARTE0>;
    RNG => rng::InterruptHandler<peripherals::RNG>;
});

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BootState {
    pub offset: u32,
    pub actual_sector: u32,
    pub actual_pkt_idx: u32,
}

fn update_chunk<'a>(boot_status: &'a mut BootState, idx: usize, data: &'a [u8], flash: &'a mut Nvmc, tx: &mut UarteTx<UARTE0>) {
    // Check what sector we are in now
    // Increase offset with data len
    let cursor = BASE_APP_ADDR + boot_status.offset;
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
            // Print some infos on update
            boot_status.actual_sector = BASE_APP_ADDR + (boot_status.offset / FLASH_PAGE) * FLASH_PAGE;
            info!("Updating flash page starting at addr: {:02X}", boot_status.actual_sector);
            info!("offset : {:02X}", boot_status.actual_sector + boot_status.offset % FLASH_PAGE);
            let crc = Crc::<u32>::new(&CRC_32_ISCSI);
            let crc_pkt = crc.checksum(data);
            // Align packet index to avoid double send of yet flashed packet
            boot_status.actual_pkt_idx = idx as u32;
            // If write chunck is ok ack
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

pub fn ack_msg_send(message: HostProtocolMessage, tx: &mut UarteTx<UARTE0>) {
    // Prepare cobs buffer
    let mut buf_cobs = [0_u8; 64];
    let cobs_ack = to_slice_cobs(&message, &mut buf_cobs).unwrap();

    let _ = tx.blocking_write(cobs_ack);
}

#[cfg(feature = "flash-protect")]
// Flash areas protection using https://docs.nordicsemi.com/bundle/ps_nrf52805/page/bprot.html
fn flash_protect() {
    // Set bprot registers values
    // Nordic MBR area protection
    // let bits_0 = unsafe { &*nrf52805_pac::BPROT::ptr() }.config0.read().bits();
    unsafe { &*nrf52805_pac::BPROT::ptr() }.config0.write(|w| w.region0().enabled());
    let bits_0 = unsafe { &*nrf52805_pac::BPROT::ptr() }.config0.read().bits();
    info!("CONFIG0_BITS : {}", bits_0);

    // Bootloader area protection
    // let bits_1 = unsafe { &*nrf52805_pac::BPROT::ptr() }.config1.read().bits();
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

    // Enable area protection also in debug
    // let disabledebug = unsafe { &*nrf52805_pac::BPROT::ptr() }.disableindebug.read().bits();
    unsafe { &*nrf52805_pac::BPROT::ptr() }
        .disableindebug
        .write(|w| unsafe { w.bits(0x00) });
    let disabledebug = unsafe { &*nrf52805_pac::BPROT::ptr() }.disableindebug.read().bits();
    info!("DISABLE : {}", disabledebug);
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    #[cfg(feature = "flash-protect")]
    flash_protect();

    let p = embassy_nrf::init(Default::default());

    let mut config_uart = uarte::Config::default();
    config_uart.parity = uarte::Parity::EXCLUDED;
    config_uart.baudrate = uarte::Baudrate::BAUD115200;

    // Uarte config
    let uart = uarte::Uarte::new(p.UARTE0, Irqs, p.P0_16, p.P0_18, config_uart);
    let (mut tx, mut rx) = uart.split_with_idle(p.TIMER0, p.PPI_CH0, p.PPI_CH1);

    // RNG - sync
    let rng = Rng::new(p.RNG, Irqs);
    {
        RNG_HW.lock(|f| f.borrow_mut().replace(rng));
    }

    // FLASH
    let mut flash = Nvmc::new(p.NVMC);

    // Valid firmware flag
    let mut fw_is_valid = false;

    // Check fw at startup
    {
        // Get Cosign application header if present
        let image_slice = get_fw_image_slice(BASE_APP_ADDR, APP_SIZE);
        // Check if fw is valid
        if let Some(res) = check_fw(image_slice, &mut tx) {
            fw_is_valid = res;
            info!("fw is : {}", res);
        } else {
            ack_msg_send(HostProtocolMessage::Bootloader(Bootloader::NoCosignHeader), &mut tx);
            info!("shit ");
        }
    }

    // Check secret seal
    let seal = unsafe { &*nrf52805_pac::UICR::ptr() }.customer[SEAL_IDX].read().customer().bits();

    // Init a GPIO to use as bootloader trigger
    let boot_gpio = Input::new(p.P0_20, Pull::Down);
    // Small delay to have stable GPIO
    let _ = Timer::after_micros(5).await;

    // // Message must be in SRAM
    let mut buf = [0; 22];
    buf.copy_from_slice(b"Hello from bootloader!");
    let _ = tx.write(&buf).await;

    // Keep track of update of flash Application
    let mut boot_status: BootState = Default::default();

    let mut jump_app = false;

    // Loop for bootloader commands
    // This loop will be a while loop with gpio state as condition to exit...
    // while boot_gpio.is_high() && !fw_is_valid{
    'exitloop: while !jump_app {
        // Now for testing locally i am looping until command reset
        // Raw buffer - 32 bytes for the accumulator of cobs
        let mut raw_buf = [0u8; 64];
        // Create a cobs accumulator for data incoming
        let mut cobs_buf: CobsAccumulator<COBS_MAX_MSG_SIZE> = CobsAccumulator::new();
        // Getting chars from Uart in a while loop
        while let Ok(n) = rx.read_until_idle(&mut raw_buf).await {
            // Finished reading input
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
                        new_wind
                    }
                    FeedResult::DeserError(new_wind) => {
                        info!("DeserError");
                        new_wind
                    }
                    FeedResult::Success { data, remaining } => {
                        info!("Remaining {} bytes", remaining.len());

                        match data {
                            HostProtocolMessage::Bluetooth(_) => (), // Message for application
                            HostProtocolMessage::Bootloader(boot_msg) => match boot_msg {
                                // Erase all flash application space
                                Bootloader::EraseFirmware => {
                                    info!("Erase firmware");
                                    if flash.erase(BASE_APP_ADDR, BASE_BOOTLOADER_APP).is_ok() {
                                        ack_msg_send(HostProtocolMessage::Bootloader(Bootloader::AckEraseFirmware), &mut tx);
                                        //Reset counters
                                        boot_status = Default::default();
                                    }
                                }
                                // Write chunks of firmware
                                Bootloader::WriteFirmwareBlock {
                                    block_idx: idx,
                                    block_data: data,
                                } => {
                                    update_chunk(&mut boot_status, idx, data, &mut flash, &mut tx);
                                }
                                Bootloader::FirmwareVersion => {
                                    let header_slice = get_fw_image_slice(BASE_APP_ADDR, APP_SIZE);
                                    if let Ok(Some(header)) = Header::parse_unverified(header_slice) {
                                        let version = header.version();
                                        ack_msg_send(HostProtocolMessage::Bootloader(Bootloader::AckFirmwareVersion { version }), &mut tx)
                                    } else {
                                        ack_msg_send(HostProtocolMessage::Bootloader(Bootloader::NoCosignHeader), &mut tx);
                                    }
                                }
                                Bootloader::BootloaderVersion => {
                                    let version = env!("CARGO_PKG_VERSION");
                                    ack_msg_send(
                                        HostProtocolMessage::Bootloader(Bootloader::AckBootloaderVersion { version }),
                                        &mut tx,
                                    )
                                }
                                Bootloader::VerifyFirmware => {
                                    let image_slice = get_fw_image_slice(BASE_APP_ADDR, APP_SIZE);
                                    info!("Image slice len dec {} - hex {:02X}", image_slice.len(), image_slice.len());
                                    if let Some(res) = check_fw(image_slice, &mut tx) {
                                        info!("Fw image valid : {}", res);
                                        fw_is_valid = true;
                                    } else {
                                        info!("No Header present!");
                                        ack_msg_send(HostProtocolMessage::Bootloader(Bootloader::NoCosignHeader), &mut tx);
                                    }
                                }
                                Bootloader::ChallengeSet { secret } => {
                                    info!("Challenge set cmd rx");
                                    // Check if we yet sealed the secret
                                    let result = if seal == SEALED_SECRET {
                                        SecretSaveResponse::NotAllowed
                                    } else {
                                        // Save secret!
                                        unsafe {
                                            match write_secret(secret) {
                                                true => {
                                                    info!("Saved!");
                                                    SecretSaveResponse::Sealed
                                                }
                                                false => SecretSaveResponse::Error,
                                            }
                                        }
                                    };
                                    // Send result to MCU
                                    ack_msg_send(HostProtocolMessage::Bootloader(Bootloader::AckChallengeSet { result }), &mut tx);
                                }
                                Bootloader::ChallengeRequest { challenge, nonce } => {
                                    let data = sha { sha: [0; 32] };
                                    let challenge_sha: &dyn Sha256 = &data;
                                    let val = unsafe { &*nrf52805_pac::UICR::ptr() }.customer[challenge].read().customer().bits();
                                    let result = challenge_sha.hash(&val.to_be_bytes());
                                    ack_msg_send(HostProtocolMessage::Bootloader(Bootloader::ChallengeResult { result }), &mut tx);
                                }
                                _ => (),
                            },
                            HostProtocolMessage::Reset => {
                                jump_app = true;
                                break 'exitloop;
                            }
                        };
                        remaining
                    }
                };
            }
            embassy_time::Timer::after_millis(1).await;
        }
    }
    drop(tx);
    drop(rx);
    unsafe {
        jump_to_app();
    }
}
