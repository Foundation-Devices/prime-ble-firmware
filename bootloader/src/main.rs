// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]
#![no_main]
mod verify;
mod jump_app;

use defmt_rtt as _;
use embassy_nrf::peripherals::{self, RNG};
// global logger
use embassy_nrf as _;
// time driver
use panic_probe as _;

use core::cell::RefCell;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use crc::{Crc, CRC_32_ISCSI};
use defmt::info;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Input, Pull};
use embassy_nrf::nvmc::Nvmc;
use embassy_nrf::rng;
use embassy_nrf::rng::Rng;
use embassy_nrf::{bind_interrupts, uarte};
use embedded_storage::nor_flash::NorFlash;
use host_protocol::Bootloader::{self, AckWithIdxCrc, NackWithIdx};
use host_protocol::HostProtocolMessage;
use postcard::accumulator::{CobsAccumulator, FeedResult};
use postcard::to_slice_cobs;
use serde::{Deserialize, Serialize};
use verify::verify_os_image;
use jump_app::jump_to_app;

// Mutex for random hw generator to delay in verification
static RNG_HW: Mutex<ThreadModeRawMutex, RefCell<Option<Rng<'_,RNG>>>> = Mutex::new(RefCell::new(None));

bind_interrupts!(struct Irqs {
    UARTE0_UART0 => uarte::InterruptHandler<peripherals::UARTE0>;
    RNG => rng::InterruptHandler<peripherals::RNG>;
});

#[used]
#[link_section = ".uicr_bootloader_start_address"]
pub static BOOTLOADER_ADDR: i32 = 0x27000;

const BASE_ADDRESS_APP: u32 = 0x19000;
const BASE_BOOTLOADER_APP: u32 = 0x27000;
const FLASH_PAGE: u32 = 4096;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BootState {
    pub offset: u32,
    pub actual_sector: u32,
    pub actual_pkt_idx: u32,
}

fn update_chunk<'a>(
    boot_status: &'a mut BootState,
    idx: usize,
    data: &'a [u8],
    flash: &'a mut Nvmc,
) -> HostProtocolMessage<'a> {
    // Check what sector we are in now
    // Increase offset with data len
    let cursor = BASE_ADDRESS_APP + boot_status.offset;
    match cursor {
        (BASE_ADDRESS_APP..=BASE_BOOTLOADER_APP) => {}
        _ => {
            return HostProtocolMessage::Bootloader(Bootloader::FirmwareOutOfBounds {
                block_idx: idx,
            })
        }
    }

    let ack = match flash.write(cursor, data) {
        Ok(()) => {
            boot_status.offset += data.len() as u32;
            // Print some infos on update
            boot_status.actual_sector =
                BASE_ADDRESS_APP + (boot_status.offset / FLASH_PAGE) * FLASH_PAGE;
            info!(
                "Updating flash page starting at addr: {:02X}",
                boot_status.actual_sector
            );
            info!(
                "offset : {:02X}",
                boot_status.actual_sector + boot_status.offset % 4096
            );
            let crc = Crc::<u32>::new(&CRC_32_ISCSI);
            let crc_pkt = crc.checksum(data);
            // Align packet index to avoid double send of yet flashed packet
            boot_status.actual_pkt_idx = idx as u32;
            // If write chunck is ok ack
            HostProtocolMessage::Bootloader(AckWithIdxCrc {
                block_idx: idx,
                crc: crc_pkt,
            })
        }
        Err(_) => HostProtocolMessage::Bootloader(NackWithIdx { block_idx: idx }),
    };
    ack
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_nrf::init(Default::default());

    let mut config_uart = uarte::Config::default();
    config_uart.parity = uarte::Parity::EXCLUDED;
    config_uart.baudrate = uarte::Baudrate::BAUD115200;

    // Uarte config
    let uart = uarte::Uarte::new(p.UARTE0, Irqs, p.P0_16, p.P0_18, config_uart);
    let (mut tx, mut rx) = uart.split_with_idle(p.TIMER0, p.PPI_CH0, p.PPI_CH1);
    
    // RNG
    let rng = Rng::new(p.RNG, Irqs);
    {
        RNG_HW.lock(|f| {
            f.borrow_mut().replace(rng)
        });
    }

    // FLASH
    let mut flash = Nvmc::new(p.NVMC);

    // Init a GPIO to use as bootloader trigger
    let _boot_gpio = Input::new(p.P0_11, Pull::Up);

    // // Message must be in SRAM
    let mut buf = [0; 22];
    buf.copy_from_slice(b"Hello from bootloader!");
    let _ = tx.write(&buf).await;

    // Keep track of update of flash Application
    let mut boot_status: BootState = Default::default();

    // Loop for bootloader commands
    // This loop will be a while loop with gpio state as condition to exit...
    loop {
        // Raw buffer - 32 bytes for the accumulator of cobs
        let mut raw_buf = [0u8; 512];
        // Create a cobs accumulator for data incoming
        let mut cobs_buf: CobsAccumulator<512> = CobsAccumulator::new();
        // Getting chars from Uart in a while loop
        while let Ok(n) = rx.read_until_idle(&mut raw_buf).await {
            // Finished reading input
            if n == 0 {
                info!("Read 0 bytes");
                break;
            }
            info!("Data incoming {}", n);

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
                                Bootloader::EraseFirmware => {
                                    info!("Erase firmware");
                                    let _ = flash.erase(BASE_ADDRESS_APP, BASE_BOOTLOADER_APP);
                                    //Reset counters
                                    boot_status = Default::default();
                                }
                                Bootloader::WriteFirmwareBlock {
                                    block_idx: idx,
                                    block_data: data,
                                } => {
                                    info!("Bootloader pkt recv");
                                    // cobs buffer for acks
                                    let mut buf_cobs = [0_u8; 16];
                                    let ack = update_chunk(&mut boot_status, idx, data, &mut flash);
                                    let cobs_ack = to_slice_cobs(&ack, &mut buf_cobs).unwrap();
                                    let _ = tx.blocking_write(cobs_ack);
                                }
                                Bootloader::VerifyFirmware => {
                                    let image_slice = unsafe {
                                        core::slice::from_raw_parts(
                                            BASE_ADDRESS_APP as *const u8,
                                            boot_status.offset as usize,
                                        )
                                    };
                                    info!("Image slice len dec {} - hex {:02X}", image_slice.len(),image_slice.len());
                                    let _ = verify_os_image(image_slice);
                                    //Reset counters
                                    boot_status = Default::default();
                                }
                                _ => (),
                            },
                            HostProtocolMessage::Reset => {
                                info!("Resetting");
                                drop(tx);
                                drop(rx);
                                unsafe {
                                    jump_to_app();
                                }
                            }
                        };
                        remaining
                    }
                };
            }
            embassy_time::Timer::after_millis(1).await;
        }
    }
    unsafe {
        jump_to_app();
    }
}
