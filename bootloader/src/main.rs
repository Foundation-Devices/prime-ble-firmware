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
use embassy_nrf::interrupt;
use embassy_nrf::{bind_interrupts, uarte};
use embassy_nrf::gpio::{Input, Pull};


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
pub static  BOOTLOADER_ADDR : i32 = 0x28000;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    
   
    let mut conf = embassy_nrf::config::Config::default(); 
    conf.gpiote_interrupt_priority = interrupt::Priority::P2;
    conf.time_interrupt_priority = interrupt::Priority::P2;

    let p = embassy_nrf::init(conf);
    Timer::after_millis(1000).await;

    let mut config_uart = uarte::Config::default();
    config_uart.parity = uarte::Parity::EXCLUDED;
    config_uart.baudrate = uarte::Baudrate::BAUD115200;

    let mut uart = uarte::Uarte::new(p.UARTE0, Irqs, p.P0_16, p.P0_18, config_uart);

    let _boot_gpio = Input::new(p.P0_11, Pull::Up);

    // // Message must be in SRAM
    let mut buf = [0; 22];
    buf.copy_from_slice(b"Hello from bootloader!");

    unwrap!(uart.write(&buf).await);

    
    let mut countdown_boot : u8 = 10;

    loop{
    // while btn1.is_low() {
        Timer::after_millis(1000).await;
        info!("Going to app in {} seconds..", countdown_boot);
        countdown_boot -= 1;

        if countdown_boot == 0{
            break;
        }
    }
    info!("going to app");
    unsafe {jump_to_app();}

}
