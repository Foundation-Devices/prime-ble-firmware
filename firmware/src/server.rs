// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

use core::pin::pin;

use crate::{nus::*, BT_ADV_CHAN, BT_ADV_CHANGED, BT_ENABLE, CONNECTION, DEVICE_NAME, TX_PWR_VALUE};
use consts::{ATT_MTU, MAX_DEVICE_NAME_LEN, SERVICES_LIST, SHORT_NAME};
use defmt::{debug, error, info, unwrap};
use nrf_softdevice::ble::advertisement_builder::{
    AdvertisementBuilder, Flag, LegacyAdvertisementBuilder, LegacyAdvertisementPayload, ServiceList,
};
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

static DEVICE_NAME_SEC_MODE: raw::ble_gap_conn_sec_mode_t = raw::ble_gap_conn_sec_mode_t {
    // Security Mode 0 Level 0: No write access
    _bitfield_1: raw::__BindgenBitfieldUnit::new([0x00]),
};

#[gatt_server]
pub struct Server {
    nus: Nus,
}

impl Server {
    pub fn send_notify<'a>(&self, connection: &'a Connection, buffer: &[u8]) -> Result<(), NotifyValueError> {
        notify_value(connection, self.nus.get_handle(), &buffer)
    }
}

#[allow(static_mut_refs)]
pub async fn initialize_sd() -> &'static mut Softdevice {
    static mut DEVICE_NAME_STORAGE: [u8; MAX_DEVICE_NAME_LEN] = [0; MAX_DEVICE_NAME_LEN];
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
        gap_device_name: Some(unsafe {
            raw::ble_gap_cfg_device_name_t {
                p_value: DEVICE_NAME_STORAGE.as_ptr() as _,
                current_len: 0,
                max_len: MAX_DEVICE_NAME_LEN as u16,
                write_perm: DEVICE_NAME_SEC_MODE,
                _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(raw::BLE_GATTS_VLOC_USER as u8),
            }
        }),
        conn_gatts: Some(raw::ble_gatts_conn_cfg_t { hvn_tx_queue_size: 3 }),

        ..Default::default()
    };

    Softdevice::enable(&config)
}

async fn run_bluetooth_inner(sd: &'static Softdevice, server: &Server) -> ! {
    static ADV_DATA: LegacyAdvertisementPayload = LegacyAdvertisementBuilder::new()
        .flags(&[Flag::GeneralDiscovery, Flag::LE_Only])
        .services_128(ServiceList::Complete, &SERVICES_LIST)
        .short_name(SHORT_NAME)
        .build();

    loop {
        BT_ADV_CHANGED.reset();
        const MAX_ADVERTISEMENT_LEN: usize = MAX_DEVICE_NAME_LEN + 2;
        let scan_data = {
            let device_name = DEVICE_NAME.lock().await;
            unsafe { raw::sd_ble_gap_device_name_set(&DEVICE_NAME_SEC_MODE, device_name.0.as_ptr(), device_name.1 as u16) };
            AdvertisementBuilder::<MAX_ADVERTISEMENT_LEN>::new()
                .full_name(str::from_utf8(&device_name.0[..device_name.1]).unwrap_or(SHORT_NAME))
                .build()
        };

        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data: &ADV_DATA,
            scan_data: &scan_data,
        };
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

        let advertise_fut = peripheral::advertise_connectable(sd, adv, &config);
        let adv_changed_fut = BT_ADV_CHANGED.wait();
        // Start advertising
        let mut conn = match futures::future::select(pin!(advertise_fut), adv_changed_fut).await {
            futures::future::Either::Left((conn, _)) => unwrap!(conn, "Advertise failed"),
            futures::future::Either::Right(((), _)) => {
                info!("Advertisement data changed, restarting");
                continue;
            }
        };

        info!("advertising done!");

        let gap_conn_param = ble_gap_conn_params_t {
            conn_sup_timeout: 500,         // 5s
            max_conn_interval: ci_ms!(50), // 50ms
            min_conn_interval: ci_ms!(5),  // 5ms
            slave_latency: 0,
        };
        // Request connection param update
        if conn.set_conn_params(gap_conn_param).is_err() {
            error!("set_conn_params error")
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
            let _ = gatt_server::run(conn, server, |e| server.handle_event(e)).await;
            info!("gatt_server run exited");
        }
        *CONNECTION.write().await = None;
    }
}

pub async fn run_bluetooth(sd: &'static Softdevice, server: &Server) -> ! {
    loop {
        // Wait for start signal
        while !BT_ENABLE.wait().await {}
        let run_bluetooth_fut = run_bluetooth_inner(sd, &server);
        let check_stopped_fut = async { while BT_ENABLE.wait().await {} };

        info!("Starting BLE advertisement");
        // source of this idea https://github.com/embassy-rs/nrf-softdevice/blob/master/examples/src/bin/ble_peripheral_onoff.rs
        futures::future::select(pin!(run_bluetooth_fut), pin!(check_stopped_fut)).await;
    }
}

impl Server {
    fn handle_event(&self, event: ServerEvent) {
        match event {
            ServerEvent::Nus(e) => self.nus.handle(e),
        }
    }
}
