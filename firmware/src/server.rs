// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{nus::*, CONNECTION};
use crate::{BT_ADV_CHAN, BT_STATE, TX_PWR_VALUE};
use consts::{ATT_MTU, DEVICE_NAME, SERVICES_LIST, SHORT_NAME};
use core::mem;
use defmt::{debug, error, info, unwrap};
use embassy_time::Timer;
use futures::pin_mut;
use nrf_softdevice::ble::advertisement_builder::{ExtendedAdvertisementBuilder, ExtendedAdvertisementPayload, Flag, ServiceList};
use nrf_softdevice::ble::gatt_server::{notify_value, NotifyValueError};
use nrf_softdevice::ble::peripheral;
use nrf_softdevice::ble::{gatt_server, Connection, TxPower};
use nrf_softdevice::gatt_server;
use nrf_softdevice::{raw, Softdevice};
use raw::ble_gap_conn_params_t;

// Get connection interval with macro
// to get 15ms just call ci_ms!(15)
macro_rules! ci_ms {
    ($a:expr) => {{
        let ms = ($a as f32 * 1000.0) / 1250.0;
        debug!("ci units: {}", ms);
        ms as u16
    }};
}

#[gatt_server]
pub struct Server {
    nus: Nus,
}

impl Server {
    pub fn send_notify<'a>(&self, connection: &'a Connection, buffer: &[u8]) -> Result<(), NotifyValueError> {
        notify_value(connection, self.nus.get_handle(), &buffer)
    }
}

async fn stop_bluetooth() {
    while BT_STATE.load(core::sync::atomic::Ordering::Relaxed) {
        // Do nothing
        Timer::after_millis(200).await;
    }
    info!("BT off");
}

pub fn initialize_sd() -> &'static mut Softdevice {
    let config = nrf_softdevice::Config {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_XTAL as u8,
            rc_ctiv: 0,
            rc_temp_ctiv: 0,
            accuracy: raw::NRF_CLOCK_LF_ACCURACY_20_PPM as u8,
        }),
        conn_gap: Some(raw::ble_gap_conn_cfg_t {
            conn_count: 1,
            event_length: 400,
        }),
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t { att_mtu: ATT_MTU as u16 }),
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
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(raw::BLE_GATTS_VLOC_USER as u8),
        }),
        conn_gatts: Some(raw::ble_gatts_conn_cfg_t { hvn_tx_queue_size: 3 }),

        ..Default::default()
    };

    Softdevice::enable(&config)
}

async fn run_bluetooth_inner(sd: &'static Softdevice, server: &Server) {
    static ADV_DATA: ExtendedAdvertisementPayload = ExtendedAdvertisementBuilder::new()
        .flags(&[Flag::GeneralDiscovery, Flag::LE_Only])
        .services_128(ServiceList::Complete, &SERVICES_LIST)
        .short_name(SHORT_NAME)
        .build();

    static SCAN_DATA: ExtendedAdvertisementPayload = ExtendedAdvertisementBuilder::new().full_name(DEVICE_NAME).build();

    let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
        adv_data: &ADV_DATA,
        scan_data: &SCAN_DATA,
    };

    loop {
        // Set advertising timer in units of 625us (about 50ms with 75 units)
        let config = peripheral::Config {
            interval: 75,
            channel_mask: [0, 0, 0, 0, BT_ADV_CHAN.load(core::sync::atomic::Ordering::Relaxed)],
            tx_power: match TX_PWR_VALUE.load(core::sync::atomic::Ordering::Relaxed) {
                -40 => TxPower::Minus40dBm,
                -20 => TxPower::Minus20dBm,
                -16 => TxPower::Minus16dBm,
                -12 => TxPower::Minus12dBm,
                -8 => TxPower::Minus8dBm,
                -4 => TxPower::Minus4dBm,
                3 => TxPower::Plus3dBm,
                4 => TxPower::Plus4dBm,
                _ => TxPower::ZerodBm,
            },
            ..Default::default()
        };

        // Start advertising
        let mut conn = unwrap!(peripheral::advertise_connectable(sd, adv, &config).await);
        info!("advertising done!");

        let gap_conn_param = ble_gap_conn_params_t {
            conn_sup_timeout: 500,         // 5s
            max_conn_interval: ci_ms!(50), // 50ms
            min_conn_interval: ci_ms!(5),  // 5ms
            slave_latency: 0,
        };
        // Request connection param update
        if let Err(e) = conn.set_conn_params(gap_conn_param) {
            error!("set_conn_params error - {:?}", e)
        }

        // Enable to biggest LL payload size to optimize BLE throughput
        if conn
            .data_length_update(Some(&raw::ble_gap_data_length_params_t {
                max_tx_octets: 251,
                max_rx_octets: 251,
                max_tx_time_us: 0,
                max_rx_time_us: 0,
            }))
            .is_err()
        {
            error!("data_length_update error");
        };

        // Start rssi capture
        conn.start_rssi();

        *CONNECTION.write().await = Some(conn);
        {
            let conn_lock = CONNECTION.read().await;
            let Some(conn) = conn_lock.as_ref() else {
                error!("Connection disappeared");
                continue;
            };
            let e = gatt_server::run(conn, server, |e| server.handle_event(e)).await;
            info!("gatt_server run exited: {:?}", e);
        }
        *CONNECTION.write().await = None;
    }
}

pub async fn run_bluetooth(sd: &'static Softdevice, server: &Server) -> ! {
    loop {
        if BT_STATE.load(core::sync::atomic::Ordering::Relaxed) {
            let run_bluetooth_fut = run_bluetooth_inner(sd, &server);
            let stop_bluetooth_fut = stop_bluetooth();
            pin_mut!(run_bluetooth_fut);
            pin_mut!(stop_bluetooth_fut);

            info!("Starting BLE advertisement");
            // source of this idea https://github.com/embassy-rs/nrf-softdevice/blob/master/examples/src/bin/ble_peripheral_onoff.rs
            futures::future::select(run_bluetooth_fut, stop_bluetooth_fut).await;
        }
        Timer::after_millis(200).await;
    }
}

impl Server {
    fn handle_event(&self, event: ServerEvent) {
        match event {
            ServerEvent::Nus(e) => self.nus.handle(e),
        }
    }
}
