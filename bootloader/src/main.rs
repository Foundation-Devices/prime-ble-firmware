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


use defmt::{info, *};
use embassy_executor::Spawner;
use embassy_time::{Duration,with_timeout};
use embassy_nrf::interrupt;
use embassy_nrf::{bind_interrupts, uarte};
use embassy_nrf::gpio::{Input, Pull};
use postcard::accumulator::{CobsAccumulator, FeedResult};
use postcard::to_slice_cobs;
use host_protocol::HostProtocolMessage;



bind_interrupts!(struct Irqs {
    UARTE0_UART0 => uarte::InterruptHandler<peripherals::UARTE0>;
});


/// Boots the application assuming softdevice is present.
    ///
    /// # Safety
    ///
    /// This modifies the stack pointer and reset vector and will run code placed in the active partition.
pub unsafe fn jump_to_app() -> ! {
    use nrf_softdevice_mbr as mbr;
    use nrf_softdevice::Softdevice;

    // Address of softdevice which we'll forward interrupts to
    let addr = 0x1000;
    let mut cmd = mbr::sd_mbr_command_t {
        command: mbr::NRF_MBR_COMMANDS_SD_MBR_COMMAND_IRQ_FORWARD_ADDRESS_SET,
        params: mbr::sd_mbr_command_t__bindgen_ty_1 {
            irq_forward_address_set: mbr::sd_mbr_command_irq_forward_address_set_t { address: addr },
        },
    };
    let ret = mbr::sd_mbr_command(&mut cmd);
    info!("ret val {}",ret);

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

#[used]
#[link_section = ".uicr_bootloader_start_address"]
pub static  BOOTLOADER_ADDR : i32 = 0x2A000;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    
    let p = embassy_nrf::init(Default::default());
    Timer::after_millis(5000).await;

    let mut config_uart = uarte::Config::default();
    config_uart.parity = uarte::Parity::EXCLUDED;
    config_uart.baudrate = uarte::Baudrate::BAUD115200;

    let uart = uarte::Uarte::new(p.UARTE0, Irqs, p.P0_16, p.P0_18, config_uart);
    let (mut tx, mut rx) = uart.split_with_idle(p.TIMER0, p.PPI_CH0, p.PPI_CH1);


    let _boot_gpio = Input::new(p.P0_11, Pull::Up);

    // // Message must be in SRAM
    let mut buf = [0; 22];
    buf.copy_from_slice(b"Hello from bootloader!");
    let _ = tx.write(&buf).await;
    
    let mut countdown_boot : u8 = 10;

    loop{
    // while boot_gpio.is_low() {
        Timer::after_millis(1000).await;
        info!("Going to app in {} seconds..", countdown_boot);
        countdown_boot -= 1;

        if countdown_boot == 0{
            break;
        }

        // Raw buffer - 32 bytes for the accumulator of cobs
        let mut raw_buf = [0u8; 32];
        // Create a cobs accumulator for data incoming
        let mut cobs_buf: CobsAccumulator<32> = CobsAccumulator::new();
            // Getting chars from Uart in a while loop
            if let Ok(n) = with_timeout(Duration::from_millis(100), rx.read_until_idle(&mut raw_buf)).await{
                let n = n.unwrap();
                // Finished reading input
                if n == 0 {
                    info!("Read 0 bytes");
                    break;
                }
                info!("Data incoming {}", n);

                let buf = &raw_buf[..n];
                let mut window = buf;

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
                                HostProtocolMessage::Bootloader(_BootMsg) => {
                                    info!("Bootloader pkt recv")
                                }, // no-op, handled in the bootloader
                                HostProtocolMessage::Reset => {
                                    info!("Resetting");
                                    // TODO: reset
                                }
                            };

                            remaining
                        }
                    };
                }
            embassy_time::Timer::after_millis(1).await;
        }
    }
    info!("going to app...");
    // Jump to application
    unsafe {jump_to_app();}
}
