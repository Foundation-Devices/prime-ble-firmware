// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_nrf::peripherals;
// global logger
use embassy_nrf as _;
use embassy_time::Timer;
// time driver
use panic_probe as _;

use crc::{Crc, CRC_32_ISCSI};
use defmt::{info, *};
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Input, Pull};
use embassy_nrf::nvmc::Nvmc;
use embassy_nrf::{bind_interrupts, uarte};
use embassy_time::{with_timeout, Duration};
use embedded_storage::nor_flash::NorFlash;
use host_protocol::Bootloader::{self, AckWithIdx, AckWithIdxCrc, NackWithIdx};
use host_protocol::HostProtocolMessage;
use postcard::accumulator::{CobsAccumulator, FeedResult};
use postcard::to_slice_cobs;
use serde::{Deserialize, Serialize};

bind_interrupts!(struct Irqs {
    UARTE0_UART0 => uarte::InterruptHandler<peripherals::UARTE0>;
});

#[used]
#[link_section = ".uicr_bootloader_start_address"]
pub static BOOTLOADER_ADDR: i32 = 0x2A000;

const BASE_ADDRESS_APP: u32 = 0x19000;
const BASE_BOOTLOADER_APP: u32 = 0x2A000;
const FLASH_PAGE: u32 = 4096;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BootState {
    pub offset: u32,
    pub actual_sector: u32,
    pub actual_pkt_idx: u32,
    pub end_sector: u32,
    pub start_sector: u32,
}

/// Boots the application assuming softdevice is present.
///
/// # Safety
///
/// This modifies the stack pointer and reset vector and will run code placed in the active partition.
pub unsafe fn jump_to_app() -> ! {
    use nrf_softdevice::Softdevice;
    use nrf_softdevice_mbr as mbr;

    // Address of softdevice which we'll forward interrupts to
    let addr = 0x1000;
    let mut cmd = mbr::sd_mbr_command_t {
        command: mbr::NRF_MBR_COMMANDS_SD_MBR_COMMAND_IRQ_FORWARD_ADDRESS_SET,
        params: mbr::sd_mbr_command_t__bindgen_ty_1 {
            irq_forward_address_set: mbr::sd_mbr_command_irq_forward_address_set_t {
                address: addr,
            },
        },
    };
    let ret = mbr::sd_mbr_command(&mut cmd);
    info!("ret val {}", ret);

    let msp = *(addr as *const u32);
    let rv = *((addr + 4) as *const u32);

    trace!("msp = {=u32:x}, rv = {=u32:x}", msp, rv);

    // These instructions perform the following operations:
    //
    // * Modify control register to use MSP as stack pointer (clear spsel bit)
    // * Synchronize instruction barrier
    // * Initialize stack pointer (0x1000)
    // * Set link register to not return (0xFF)
    // * Jump to softdevice reset vector
    core::arch::asm!(
        "mrs {tmp}, CONTROL",
        "bics {tmp}, {spsel}",
        "msr CONTROL, {tmp}",
        "isb",
        "msr MSP, {msp}",
        "mov lr, {new_lr}",
        "bx {rv}",
        // `out(reg) _` is not permitted in a `noreturn` asm! call,
        // so instead use `in(reg) 0` and don't restore it afterwards.
        tmp = in(reg) 0,
        spsel = in(reg) 2,
        new_lr = in(reg) 0xFFFFFFFFu32,
        msp = in(reg) msp,
        rv = in(reg) rv,
        options(noreturn),
    );
}

fn update_chunk<'a>(
    boot_status: &'a mut BootState,
    idx: usize,
    data: &'a [u8],
    flash: &'a mut Nvmc,
) -> HostProtocolMessage<'a> {
    
    // Check what sector we are in now
    boot_status.actual_sector = boot_status.offset / FLASH_PAGE;

    info!("Actual_sector : {}", boot_status.actual_sector);
    // if boot_status.actual_sector != boot_status.offset / FLASH_PAGE {
    //     boot_status.actual_sector = boot_status.offset / FLASH_PAGE;
    //     boot_status.offset = 0;
    // }

    // Increase offset with data len
    let cursor = BASE_ADDRESS_APP + boot_status.offset;

    let ack = match flash.write(cursor, data) {
        Ok(()) => {
            boot_status.offset += data.len() as u32;
            info!("New offset : {}", boot_status.offset);
            let crc = Crc::<u32>::new(&CRC_32_ISCSI);
            let crc_pkt = crc.checksum(&data);
            // Align packet index to avoid double send of yet flashed packet
            boot_status.actual_pkt_idx = idx as u32;
            // If write chunck is ok ack
            HostProtocolMessage::Bootloader(AckWithIdxCrc {
                    block_idx: idx,
                    crc: crc_pkt,
                })
        }
        Err(_) => HostProtocolMessage::Bootloader(NackWithIdx { block_idx: idx })
    };
    ack
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_nrf::init(Default::default());
    
    let mut config_uart = uarte::Config::default();
    config_uart.parity = uarte::Parity::EXCLUDED;
    config_uart.baudrate = uarte::Baudrate::BAUD115200;

    let uart = uarte::Uarte::new(p.UARTE0, Irqs, p.P0_16, p.P0_18, config_uart);
    let (mut tx, mut rx) = uart.split_with_idle(p.TIMER0, p.PPI_CH0, p.PPI_CH1);

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


    loop {
        // while boot_gpio.is_high() {
        // Timer::after_millis(1000).await;
        // info!("Going to app in {} seconds..", countdown_boot);
        // countdown_boot -= 1;

        // if countdown_boot == 0{
        //     break;
        // }

        // Raw buffer - 32 bytes for the accumulator of cobs
        let mut raw_buf = [0u8; 512];
        // Create a cobs accumulator for data incoming
        let mut cobs_buf: CobsAccumulator<512> = CobsAccumulator::new();
        // Getting chars from Uart in a while loop
        while let Ok(n) = rx.read_until_idle(&mut raw_buf).await
        {
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
                            HostProtocolMessage::Bluetooth(_) => (),
                            HostProtocolMessage::Bootloader(boot_msg) => match boot_msg {
                                Bootloader::EraseFirmware => {
                                    info!("Erase firmware");
                                    let _ = flash.erase(BASE_ADDRESS_APP, BASE_BOOTLOADER_APP);
                                }
                                Bootloader::WriteFirmwareBlock {
                                    block_idx: idx,
                                    block_data: data,
                                } => {
                                    info!("Bootloader pkt recv");
                                    // cobs buffer for acks
                                    let mut buf_cobs = [0_u8;16];
                                    let ack =
                                        update_chunk(&mut boot_status, idx, data, &mut flash);
                                    let cobs_ack = to_slice_cobs(&ack, &mut buf_cobs).unwrap();
                                    let _ = tx.blocking_write(cobs_ack);
                                }
                                _ => (),
                            }, // no-op, handled in the bootloader
                            HostProtocolMessage::Reset => {
                                info!("Resetting");
                                info!("going to app...");
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
