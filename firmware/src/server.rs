// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::consts::{ATT_MTU, DEVICE_NAME, SERVICES_LIST, SHORT_NAME};
use crate::nus::*;
use crate::TX_BT_VEC;
use crate::{BT_STATE, RSSI_VALUE};
use core::mem;
use defmt::{info, *};
use embassy_time::{Duration, Timer};
use futures::future::{select, Either};
use futures::pin_mut;
use nrf_softdevice::ble::advertisement_builder::{ExtendedAdvertisementBuilder, ExtendedAdvertisementPayload, Flag, ServiceList};
use nrf_softdevice::ble::gatt_server::notify_value;
use nrf_softdevice::ble::peripheral;
use nrf_softdevice::ble::PhySet;
use nrf_softdevice::ble::{gatt_server, Connection};
use nrf_softdevice::gatt_server;
use nrf_softdevice::{raw, Softdevice};
use raw::ble_gap_conn_params_t;


// Get connection interval with macro
// to get 15ms just call ci_ms!(15)
macro_rules! ci_ms {
    ($a:expr) => {{
        $a * 1000 / 1250
    }};
}

#[gatt_server]
pub struct Server {
    nus: Nus,
}

pub async fn stop_bluetooth() {
    info!("Waiting off");
    while BT_STATE.wait().await {}
    info!("off");
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
            let mut buffer = TX_BT_VEC.lock().await;
            if buffer.len() > 2 {
                info!("Buffer to BT len {}", buffer.len());
            }
            if buffer.len() > 0 {
                match notify_value(connection, server.nus.get_handle(), &buffer[0]) {
                    Ok(_) => {
                        buffer.remove(0);
                    }
                    Err(e) => info!("Error on nus send {:?}", e),
                }
            }

            // info!("Getting RSSI - tick 1S");
            if connection.rssi().is_some() && buffer.len() == 0 {
                // Get as u8 rssi - receiver side will take care of cast to i8
                let rssi_as_u8 = connection.rssi().unwrap() as u8;
                let mut rssi_val = RSSI_VALUE.lock().await;
                *rssi_val = rssi_as_u8;
            }
        }

        // Sleep for one millisecond.
        Timer::after(Duration::from_nanos(10)).await
    }
}

pub async fn update_phy(mut conn: Connection) {
    // delay to avoid request during discovery services, many phones reject in this case
    Timer::after_secs(2).await;
    // Request PHY2
    if conn.phy_update(PhySet::M2, PhySet::M2).is_err() {
        info!("phy_update error");
    }
}

// Set parameter for data event extension on Sd112
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
    info!("ret from conn length {}", ret);
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
        let conn = unwrap!(peripheral::advertise_connectable(sd, adv, &config).await);
        info!("advertising done!");

        // Request connection interval - trying to request a short one.
        let conn_params = ble_gap_conn_params_t {
            conn_sup_timeout: 500,
            max_conn_interval:ci_ms!(25),
            min_conn_interval:ci_ms!(12),
            slave_latency: 0,
        };

        // Request connection param update
        if let Err(e) = conn.set_conn_params(conn_params) {
            info!("set_conn_params error - {:?}", e)
        }

        // Start rssi capture
        conn.start_rssi();
        // Activate notification on handle of nus TX
        server.nus.handle(NusEvent::TxCccdWrite { notifications: true });

        let gatt_fut = gatt_server::run(&conn, server, |e| server.handle_event(e));
        let tx_fut = notify_data_tx(server, &conn);
        // let _phy_upd = update_phy(conn.clone()).await;

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
        //server.handle_event(ServerEvent::Nus(NusEvent::TxCccdWrite { notifications: true }));
        match select(tx_fut, gatt_fut).await {
            Either::Left((_, _)) => {
                info!("Tx error")
            }
            Either::Right((e, _)) => {
                info!("gatt_server run exited with error: {:?}", e);
            }
        }
        // Force false
        BT_STATE.signal(false);
    }
}

impl Server {
    fn handle_event(&self, event: ServerEvent) {
        match event {
            ServerEvent::Nus(e) => self.nus.handle(e),
        }
    }
}
