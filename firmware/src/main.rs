// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]
#![no_main]

mod comms;
mod consts;
mod nus;
mod server;

use defmt_rtt as _;
use embassy_nrf::peripherals::{TIMER1, UARTE0};
// global logger
use embassy_nrf as _;
use embassy_time::Timer;
// time driver
use panic_probe as _;

use comms::{comms_task, send_bt_uart};
use consts::MAX_IRQ;
use defmt::{info, *};
use embassy_executor::Spawner;
use embassy_nrf::buffered_uarte::{self, BufferedUarte};
use embassy_nrf::interrupt::{self, Interrupt, InterruptExt};
use embassy_nrf::{bind_interrupts, peripherals, uarte};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use futures::pin_mut;
use heapless::Vec;
use nrf_softdevice::Softdevice;
use server::{initialize_sd, run_bluetooth, stop_bluetooth, Server};
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    UARTE0_UART0 => buffered_uarte::InterruptHandler<peripherals::UARTE0>;
});
#[allow(dead_code)]
#[derive(Default)]
pub struct BleState {
    state: bool,
    rssi: Option<i8>,
}

// Signal for BT state
static BT_STATE: Signal<ThreadModeRawMutex, bool> = Signal::new();
static BT_DATA_RX: Signal<ThreadModeRawMutex, Vec<u8, 256>> = Signal::new();
static TX_BT_VEC: Mutex<ThreadModeRawMutex, Vec<Vec<u8, 256>, 4>> = Mutex::new(Vec::new());
// static RX_BT_VEC: Channel<ThreadModeRawMutex, Vec<u8, 256>, 4> = Channel::new();
static BUFFERED_UART: Mutex<ThreadModeRawMutex, Option<BufferedUarte<UARTE0, TIMER1>>> =
    Mutex::new(None);

static RSSI_VALUE: Mutex<ThreadModeRawMutex, u8> = Mutex::new(0);

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

    let mut conf = embassy_nrf::config::Config::default(); //embassy_nrf::init(Default::default());
    conf.gpiote_interrupt_priority = interrupt::Priority::P2;
    conf.time_interrupt_priority = interrupt::Priority::P2;

    let p = embassy_nrf::init(conf);

    let mut config_uart = uarte::Config::default();
    config_uart.parity = uarte::Parity::EXCLUDED;
    config_uart.baudrate = uarte::Baudrate::BAUD115200;

    static TX_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();
    static RX_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();

    //let uart = uarte::Uarte::new(p.UARTE0, Irqs, p.P0_16, p.P0_18, config_uart);
    let uart = BufferedUarte::new(
        p.UARTE0,
        p.TIMER1,
        p.PPI_CH0,
        p.PPI_CH1,
        p.PPI_GROUP0,
        Irqs,
        p.P0_16,
        p.P0_18,
        config_uart,
        &mut TX_BUFFER.init([0; 256])[..],
        &mut RX_BUFFER.init([0; 256])[..],
    );

    // Mutex is released
    {
        *(BUFFERED_UART.lock().await) = Some(uart);
    }

    // set priority to avoid collisions with softdevice
    interrupt::UARTE0_UART0.set_priority(interrupt::Priority::P3);

    let sd = initialize_sd();

    let server = unwrap!(Server::new(sd));
    unwrap!(spawner.spawn(softdevice_task(sd)));

    info!("Hello World!");

    // heartbeat small task to check activity
    unwrap!(spawner.spawn(heatbeat()));
    // Uart task
    unwrap!(spawner.spawn(comms_task()));
    unwrap!(spawner.spawn(send_bt_uart()));

    info!("Init tasks");

    for num in 0..=MAX_IRQ {
        let interrupt = unsafe { core::mem::transmute::<u16, Interrupt>(num) };
        let is_enabled = InterruptExt::is_enabled(interrupt);
        let priority = InterruptExt::get_priority(interrupt);

        defmt::println!(
            "Interrupt {}: Enabled = {}, Priority = {}",
            num,
            is_enabled,
            priority
        );
    }

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
            // info!("Init loopp");
            pin_mut!(run_bluetooth_fut);
            pin_mut!(stop_bluetooth_fut);

            info!("Starting BLE advertisment");
            // source of this idea https://github.com/embassy-rs/nrf-softdevice/blob/master/examples/src/bin/ble_peripheral_onoff.rs
            futures::future::select(run_bluetooth_fut, stop_bluetooth_fut).await;
            info!("Off Future Consumed");
        }
    }
}
