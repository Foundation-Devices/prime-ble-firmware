// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]
#![no_main]

mod consts;
mod nus;
mod server;

use cortex_m::peripheral::cpuid;
use defmt_rtt as _; use embassy_nrf::peripherals::UARTE0;
use embassy_nrf::uarte::Uarte;
// global logger
use embassy_nrf as _; 
use embassy_time::{Duration, Ticker, Timer};
// time driver
use panic_probe as _;

use core::mem;

use crate::consts::{ATT_MTU, DEVICE_NAME, SERVICES_LIST, SHORT_NAME, MAX_IRQ};
use crate::server::Server;
use defmt::{info, *};
use embassy_nrf::{bind_interrupts, peripherals, uarte};
use embassy_nrf::interrupt::{self,Interrupt, InterruptExt};
use embassy_executor::Spawner;
use nrf_softdevice::ble::advertisement_builder::{
    ExtendedAdvertisementBuilder, ExtendedAdvertisementPayload, Flag, ServiceList,
};
use nrf_softdevice::ble::{peripheral};
use nrf_softdevice::{raw, Softdevice};

bind_interrupts!(struct Irqs {
    UARTE0_UART0 => uarte::InterruptHandler<peripherals::UARTE0>;
});

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) -> ! {
    info!("SD is running");

    sd.run().await
}

#[embassy_executor::task]
async fn timer1() {
    loop {
        info!("Heartbeat - 5s");
        Timer::after_secs(5).await;
    }
}

#[embassy_executor::task]
async fn uart_test(mut uart: Uarte<'static, UARTE0>){
    let mut buf1 = [0; 8];

    loop {
        info!("reading...");
        unwrap!(uart.read(&mut buf1).await);
        info!("writing...");
        unwrap!(uart.write(&buf1).await);
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello World!");

    let config = peripheral::Config { interval: 50, ..Default::default() };
    let p = embassy_nrf::init(Default::default());

    let mut config_uart = uarte::Config::default();
    config_uart.parity = uarte::Parity::EXCLUDED;
    config_uart.baudrate = uarte::Baudrate::BAUD115200;
    let uart = uarte::Uarte::new(p.UARTE0, Irqs, p.P0_16, p.P0_18, config_uart);

    // set priority to avoid collisions with softdevice
    interrupt::UARTE0_UART0.set_priority(interrupt::Priority::P2);


    let sd = initialize_sd();

    let server = unwrap!(Server::new(sd));
    unwrap!(spawner.spawn(softdevice_task(sd)));


    for num in 0..= MAX_IRQ {
        let interrupt = unsafe { core::mem::transmute::<u16, Interrupt>(num) };
        let is_enabled = InterruptExt::is_enabled(interrupt);
        let priority = InterruptExt::get_priority(interrupt);

        defmt::println!("Interrupt {}: Enabled = {}, Priority = {}", num, is_enabled, priority);
    }

    // Message must be in SRAM
    let mut buf = [0; 8];
    buf.copy_from_slice(b"Hello!\r\n");

    unwrap!(spawner.spawn(timer1()));
    unwrap!(spawner.spawn(uart_test(uart)));

 

    static ADV_DATA: ExtendedAdvertisementPayload = ExtendedAdvertisementBuilder::new()
        .flags(&[Flag::GeneralDiscovery, Flag::LE_Only])
        .services_128(ServiceList::Complete, &SERVICES_LIST)
        .short_name(SHORT_NAME)
        .build();

    static SCAN_DATA: ExtendedAdvertisementPayload = ExtendedAdvertisementBuilder::new()
        .full_name(DEVICE_NAME)
        .build();

    let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
        adv_data: &ADV_DATA,
        scan_data: &SCAN_DATA,
    };


    loop {
        let conn = unwrap!(peripheral::advertise_connectable(sd, adv, &config).await);

        // Run the GATT server on the connection. This returns when the connection gets disconnected.
        server.run(&conn, &config).await;
    }
}

fn initialize_sd() -> &'static mut Softdevice {
    let config = nrf_softdevice::Config {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_RC as u8,
            rc_ctiv: 16,
            rc_temp_ctiv: 2,
            accuracy: raw::NRF_CLOCK_LF_ACCURACY_500_PPM as u8,
        }),
        conn_gap: Some(raw::ble_gap_conn_cfg_t {
            conn_count: 1,
            event_length: 24,
        }),
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t {
            att_mtu: ATT_MTU as u16,
        }),
        gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: raw::BLE_GATTS_ATTR_TAB_SIZE_DEFAULT,
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 1,
        }),
        gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
            p_value: DEVICE_NAME.as_ptr() as _,
            current_len: DEVICE_NAME.len() as u16,
            max_len: DEVICE_NAME.len() as u16,
            write_perm: unsafe { mem::zeroed() },
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        ..Default::default()
    };

    Softdevice::enable(&config)
}
