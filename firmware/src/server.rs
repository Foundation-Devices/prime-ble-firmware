// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::nus::*;
use crate::BT_DATA_TX;
use crate::{BT_STATE, RSSI_VALUE};
use consts::{ATT_MTU, DEVICE_NAME, SERVICES_LIST, SHORT_NAME};
use core::mem;
use defmt::{debug, error, info, unwrap};
use embassy_time::Timer;
use futures::future::{select, Either};
use futures::pin_mut;
use nrf_softdevice::ble::advertisement_builder::{ExtendedAdvertisementBuilder, ExtendedAdvertisementPayload, Flag, ServiceList};
use nrf_softdevice::ble::gatt_server::{notify_value, NotifyValueError};
use nrf_softdevice::ble::peripheral;
#[cfg(feature = "bluetooth-PHY2")]
use nrf_softdevice::ble::PhySet;
use nrf_softdevice::ble::{gatt_server, Connection};
use nrf_softdevice::gatt_server;
use nrf_softdevice::{raw, RawError, Softdevice};
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

pub async fn stop_bluetooth() {
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
            event_length: 24,
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
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(raw::BLE_GATTS_VLOC_STACK as u8),
        }),
        conn_gatts: Some(raw::ble_gatts_conn_cfg_t { hvn_tx_queue_size: 3 }),

        ..Default::default()
    };

    Softdevice::enable(&config)
}

/// Notifies the connected client about new data.
async fn notify_data_tx<'a>(server: &'a Server, connection: &'a Connection) {
    loop {
        // This is the way we can notify data when NUS service is up
        {
            let mut buffer = BT_DATA_TX.lock().await;
            if !buffer.is_empty() {
                match notify_value(connection, server.nus.get_handle(), &buffer[0]) {
                    Ok(_) => {
                        buffer.remove(0);
                    }
                    Err(NotifyValueError::Raw(RawError::BleGattsSysAttrMissing)) => {
                        // Ignore this error, no need to be spammed just because
                        // we are waiting for sys attrs to be available
                    }
                    Err(e) => error!("Error on nus send {:?}", e),
                }
            }

            // Getting RSSI if connected
            if connection.rssi().is_some() && buffer.is_empty() {
                // Get as u8 rssi - receiver side will take care of cast to i8
                let rssi_as_u8 = connection.rssi().unwrap() as u8;
                RSSI_VALUE.store(rssi_as_u8, core::sync::atomic::Ordering::Relaxed);
            }
        }

        // Sleep for one millisecond.
        Timer::after_millis(1).await
    }
}

#[cfg(feature = "bluetooth-PHY2")]
pub async fn update_phy(mut conn: Connection) {
    // delay to avoid request during discovery services, many phones reject in this case
    Timer::after_secs(2).await;
    // Request PHY2
    if conn.phy_update(PhySet::M2, PhySet::M2).is_err() {
        error!("phy_update error");
    }
}

// Set parameter for data event extension on SD112
pub fn set_data_event_ext() -> u32 {
    let ret = unsafe {
        raw::sd_ble_opt_set(
            raw::BLE_COMMON_OPTS_BLE_COMMON_OPT_CONN_EVT_EXT,
            &raw::ble_opt_t {
                common_opt: raw::ble_common_opt_t {
                    conn_evt_ext: raw::ble_common_opt_conn_evt_ext_t {
                        _bitfield_1: raw::ble_common_opt_conn_evt_ext_t::new_bitfield_1(1),
                    },
                },
            },
        )
    };
    debug!("set_data_event_ext: {}", ret);
    ret
}

pub async fn run_bluetooth(sd: &'static Softdevice, server: &Server) {
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
        set_data_event_ext();

        // Set advertising timer in units of 625us (about 50ms with 75 units)
        let config = peripheral::Config {
            interval: 75,
            ..Default::default()
        };

        // Start advertising
        #[cfg(feature = "s112")]
        let conn = unwrap!(peripheral::advertise_connectable(sd, adv, &config).await);
        #[cfg(feature = "s113")]
        let mut conn = unwrap!(peripheral::advertise_connectable(sd, adv, &config).await);
        info!("advertising done!");

        #[cfg(feature = "s112")]
        let gap_conn_param = ble_gap_conn_params_t {
            conn_sup_timeout: 500,         // 5s
            max_conn_interval: ci_ms!(10), // 15ms - having frequent connection allows to optimize BLE throughput without DLE
            min_conn_interval: ci_ms!(10), // 10ms
            slave_latency: 0,
        };
        #[cfg(feature = "s113")]
        let gap_conn_param = ble_gap_conn_params_t {
            conn_sup_timeout: 500,         // 5s
            max_conn_interval: ci_ms!(50), // 50ms
            min_conn_interval: ci_ms!(45), // 45ms - do not permit too low connection interval to not perturb BLE throughput
            slave_latency: 0,
        };
        // Request connection param update
        if let Err(e) = conn.set_conn_params(gap_conn_param) {
            error!("set_conn_params error - {:?}", e)
        }

        #[cfg(feature = "s113")]
        {
            // Enable to biggest LL payload size to optimize BLE throughput
            if conn
                .data_length_update(Some(&raw::ble_gap_data_length_params_t {
                    max_tx_octets: 251,
                    max_rx_octets: 251,
                    max_tx_time_us: 2120,
                    max_rx_time_us: 2120,
                }))
                .is_err()
            {
                error!("data_length_update error");
            };
        }

        // Start rssi capture
        conn.start_rssi();
        // Activate notification on handle of nus TX
        server.nus.handle(NusEvent::TxCccdWrite { notifications: true });

        let gatt_fut = gatt_server::run(&conn, server, |e| server.handle_event(e));
        let tx_fut = notify_data_tx(server, &conn);

        // No need to ask for PHY2, even with PHY1 the BLE maximum app data throughput (measured to 860kbps)
        // is higher than the UART maximum throughput (estimated to 360kbps at 460800bps baudrate)
        #[cfg(feature = "bluetooth-PHY2")]
        let _phy_upd = update_phy(conn.clone()).await;

        // Pin mutable futures
        pin_mut!(tx_fut);
        pin_mut!(gatt_fut);

        // We are using "select" to wait for either one of the futures to complete.
        // There are some advantages to this approach:
        //  - we only send data when a client is connected.
        //  - when the GATT server finishes operating, our ADC future is also automatically aborted.
        // Event enums (ServerEvent's) are generated by nrf_softdevice::gatt_server
        // proc macro when applied to the Server struct above
        // server.run(&conn, &config).await;
        // Turn on message on bt
        // server.handle_event(ServerEvent::Nus(NusEvent::TxCccdWrite { notifications: true }));
        match select(tx_fut, gatt_fut).await {
            Either::Left((_, _)) => {
                error!("Tx error")
            }
            Either::Right((e, _)) => {
                info!("gatt_server run exited: {:?}", e);
            }
        }
        // Force false
        BT_STATE.store(true, core::sync::atomic::Ordering::Relaxed);
        // Clear RSSI value
        RSSI_VALUE.store(0, core::sync::atomic::Ordering::Relaxed);
    }
}

impl Server {
    fn handle_event(&self, event: ServerEvent) {
        match event {
            ServerEvent::Nus(e) => self.nus.handle(e),
        }
    }
}
