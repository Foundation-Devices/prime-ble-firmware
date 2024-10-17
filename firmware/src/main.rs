// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]
#![no_main]

mod comms;
mod consts;
mod nus;
mod server;

use core::cell::RefCell;
use defmt_rtt as _;
use embassy_nrf::peripherals::{TIMER1, UARTE0};
// global logger
use embassy_nrf as _;
use embassy_time::Timer;
use embedded_io_async::Write;
// time driver
use panic_probe as _;

use comms::comms_task;
#[cfg(feature = "uart-cobs-mcu")]
use comms::send_bt_uart;
#[cfg(feature = "uart-no-cobs-mcu")]
use comms::send_bt_uart_no_cobs;
use consts::ATT_MTU;
use defmt::{info, *};
use embassy_executor::Spawner;
use embassy_nrf::buffered_uarte::{self, BufferedUarte};
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::interrupt::{self, InterruptExt};
use embassy_nrf::{bind_interrupts, peripherals, uarte};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use futures::pin_mut;
use heapless::Vec;
use host_protocol::COBS_MAX_MSG_SIZE;
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
static BT_STATE: Signal<ThreadModeRawMutex, bool> = Signal::new();
static TX_BT_VEC: Mutex<ThreadModeRawMutex, Vec<Vec<u8, ATT_MTU>, 4>> = Mutex::new(Vec::new());
static RSSI_VALUE: Mutex<ThreadModeRawMutex, u8> = Mutex::new(0);
static BT_DATA_RX: Channel<ThreadModeRawMutex, Vec<u8, ATT_MTU>, 4> = Channel::new();
static FIRMWARE_VER: Channel<ThreadModeRawMutex, &str, 1> = Channel::new();
static RSSI_TX: Channel<ThreadModeRawMutex, u8, 1> = Channel::new();
static BUFFERED_UART: StaticCell<BufferedUarte<'_, UARTE0, TIMER1>> = StaticCell::new();

/// nRF -> MPU IRQ output pin
static IRQ_OUT_PIN: Mutex<ThreadModeRawMutex, RefCell<Option<Output>>> = Mutex::new(RefCell::new(None));

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) -> ! {
    info!("SD is running");

    sd.run().await
}

#[embassy_executor::task]
async fn heartbeat() {
    loop {
        info!("Heartbeat - 30s");
        Timer::after_secs(30).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello World!");

    let mut conf = embassy_nrf::config::Config::default();
    // This caused bad behaviour at reset - will check if i did something wrong
    // conf.dcdc = embassy_nrf::config::DcdcConfig { reg1: true };
    conf.hfclk_source = embassy_nrf::config::HfclkSource::ExternalXtal;
    conf.lfclk_source = embassy_nrf::config::LfclkSource::ExternalXtal;

    conf.gpiote_interrupt_priority = interrupt::Priority::P2;
    conf.time_interrupt_priority = interrupt::Priority::P2;

    let p = embassy_nrf::init(conf);

    #[cfg(feature = "uart-pins-console")]
    let baud_rate = uarte::Baudrate::BAUD115200;

    #[cfg(feature = "uart-pins-mpu")]
    let baud_rate = uarte::Baudrate::BAUD1M;

    let mut config_uart = uarte::Config::default();
    config_uart.parity = uarte::Parity::EXCLUDED;
    config_uart.baudrate = baud_rate;

    static TX_BUFFER: StaticCell<[u8; COBS_MAX_MSG_SIZE]> = StaticCell::new();
    static RX_BUFFER: StaticCell<[u8; COBS_MAX_MSG_SIZE]> = StaticCell::new();

    #[cfg(feature = "uart-pins-mpu")]
    let (rxd, txd) = (p.P0_14, p.P0_12);

    #[cfg(feature = "uart-pins-console")]
    let (rxd, txd) = (p.P0_16, p.P0_18);

    let uart = BUFFERED_UART.init(BufferedUarte::new(
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
    ));

    let _ = uart.write_all(b"Hi from app!").await;

    let (rx, tx) = uart.split_by_ref();

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

    info!("Hello World!");

    // heartbeat small task to check activity
    unwrap!(spawner.spawn(heartbeat()));
    // Uart task
    unwrap!(spawner.spawn(comms_task(rx)));
    #[cfg(feature = "uart-cobs-mcu")]
    unwrap!(spawner.spawn(send_bt_uart(tx)));
    #[cfg(feature = "uart-no-cobs-mcu")]
    unwrap!(spawner.spawn(send_bt_uart_no_cobs(tx)));

    info!("Init tasks");

    loop {
        Timer::after_millis(100).await;
        let state = BT_STATE.wait().await;
        if state {
            info!("BT state ON");
        }
        if !state {
            info!("BT state OFF");
        }

        if state {
            let run_bluetooth_fut = run_bluetooth(sd, &server);
            let stop_bluetooth_fut = stop_bluetooth();
            pin_mut!(run_bluetooth_fut);
            pin_mut!(stop_bluetooth_fut);

            info!("Starting BLE advertisement");
            // source of this idea https://github.com/embassy-rs/nrf-softdevice/blob/master/examples/src/bin/ble_peripheral_onoff.rs
            futures::future::select(run_bluetooth_fut, stop_bluetooth_fut).await;
            info!("Off Future Consumed");
        }
    }
}
