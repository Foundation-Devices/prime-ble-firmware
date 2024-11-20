// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]
#![no_main]

mod comms;
mod consts;
mod nus;
mod server;

use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicU8};
use defmt_rtt as _;
// global logger
use embassy_nrf as _;
use embassy_time::Timer;
// time driver
use panic_probe as _;

use comms::comms_task;
#[cfg(feature = "uart-cobs-mcu")]
#[cfg(feature = "uart-no-cobs-mcu")]
use comms::send_bt_uart_no_cobs;
use consts::{ATT_MTU, BT_MAX_NUM_PKT};
use defmt::{info, *};
use embassy_executor::Spawner;
use embassy_nrf::buffered_uarte::{self, BufferedUarte};
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::interrupt::{self, InterruptExt};
use embassy_nrf::{bind_interrupts, peripherals, uarte};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use futures::pin_mut;
use heapless::Vec;
use host_protocol::COBS_MAX_MSG_SIZE;
use nrf_softdevice::ble::get_address;
use nrf_softdevice::Softdevice;
use server::{initialize_sd, run_bluetooth, stop_bluetooth, Server};
use static_cell::StaticCell;

#[cfg(all(feature = "uart-pins-console", feature = "uart-pins-mpu"))]
compile_error!("Only one of the features `uart-pins-console` or `uart-pins-mpu` can be enabled.");

#[cfg(not(any(feature = "uart-pins-console", feature = "uart-pins-mpu")))]
compile_error!("One of the features `uart-pins-console` or `uart-pins-mpu` must be enabled.");

bind_interrupts!(struct Irqs {
    UARTE0_UART0 => buffered_uarte::InterruptHandler<peripherals::UARTE0>;
});

// Signal for BT state
static BT_STATE: AtomicBool = AtomicBool::new(false);
static BT_DATA_TX: Mutex<ThreadModeRawMutex, Vec<Vec<u8, ATT_MTU>, BT_MAX_NUM_PKT>> = Mutex::new(Vec::new());
static RSSI_VALUE: AtomicU8 = AtomicU8::new(0);
static BT_DATA_RX: Channel<ThreadModeRawMutex, Vec<u8, ATT_MTU>, BT_MAX_NUM_PKT> = Channel::new();
static BT_ADDRESS: Mutex<ThreadModeRawMutex, [u8; 6]> = Mutex::new([0xFF; 6]);

/// nRF -> MPU IRQ output pin
static IRQ_OUT_PIN: Mutex<ThreadModeRawMutex, RefCell<Option<Output>>> = Mutex::new(RefCell::new(None));

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) -> ! {
    info!("SD is running");
    sd.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut conf = embassy_nrf::config::Config::default();
    // This caused bad behaviour at reset - will check if i did something wrong
    // conf.dcdc = embassy_nrf::config::DcdcConfig { reg1: true };
    conf.hfclk_source = embassy_nrf::config::HfclkSource::ExternalXtal;
    conf.lfclk_source = embassy_nrf::config::LfclkSource::ExternalXtal;

    conf.gpiote_interrupt_priority = interrupt::Priority::P2;
    conf.time_interrupt_priority = interrupt::Priority::P2;

    let p = embassy_nrf::init(conf);

    #[cfg(feature = "uart-pins-console")]
    let baud_rate = uarte::Baudrate::BAUD460800;
    #[cfg(feature = "uart-pins-console")]
    info!("Uart console pins - 460800 BAUD");

    #[cfg(feature = "uart-pins-mpu")]
    let baud_rate = uarte::Baudrate::BAUD460800;
    #[cfg(feature = "uart-pins-mpu")]
    info!("Uart MPU pins - 460800 BAUD");

    let mut config_uart = uarte::Config::default();
    config_uart.parity = uarte::Parity::EXCLUDED;
    config_uart.baudrate = baud_rate;

    static TX_BUFFER: StaticCell<[u8; COBS_MAX_MSG_SIZE]> = StaticCell::new();
    static RX_BUFFER: StaticCell<[u8; COBS_MAX_MSG_SIZE]> = StaticCell::new();

    #[cfg(feature = "uart-pins-mpu")]
    let (rxd, txd) = (p.P0_14, p.P0_12);

    #[cfg(feature = "uart-pins-console")]
    let (rxd, txd) = (p.P0_16, p.P0_18);

    let uart = BufferedUarte::new(
        p.UARTE0,
        p.TIMER1,
        p.PPI_CH0,
        p.PPI_CH1,
        p.PPI_GROUP0,
        Irqs,
        rxd,
        txd,
        config_uart,
        &mut TX_BUFFER.init([0; COBS_MAX_MSG_SIZE])[..],
        &mut RX_BUFFER.init([0; COBS_MAX_MSG_SIZE])[..],
    );

    // Configure the OUT IRQ pin
    {
        IRQ_OUT_PIN
            .lock()
            .await
            .borrow_mut()
            .replace(Output::new(p.P0_20, Level::High, OutputDrive::Standard));
    }

    // set priority to avoid collisions with softdevice
    interrupt::UARTE0_UART0.set_priority(interrupt::Priority::P3);

    let sd = initialize_sd();

    let server = unwrap!(Server::new(sd));
    unwrap!(spawner.spawn(softdevice_task(sd)));
    // Uart task
    unwrap!(spawner.spawn(comms_task(uart)));

    info!("Init tasks");

    // Get Bt device address
    let mut address = get_address(sd).bytes();
    address.reverse();
    info!("Address : {=[u8;6]:#X}", address);
    *BT_ADDRESS.lock().await = address;

    loop {
        if BT_STATE.load(core::sync::atomic::Ordering::Relaxed) {
            let run_bluetooth_fut = run_bluetooth(sd, &server);
            let stop_bluetooth_fut = stop_bluetooth();
            pin_mut!(run_bluetooth_fut);
            pin_mut!(stop_bluetooth_fut);

            info!("Starting BLE advertisement");
            // source of this idea https://github.com/embassy-rs/nrf-softdevice/blob/master/examples/src/bin/ble_peripheral_onoff.rs
            futures::future::select(run_bluetooth_fut, stop_bluetooth_fut).await;
        } else {
            Timer::after_millis(100).await;
        }
    }
}
