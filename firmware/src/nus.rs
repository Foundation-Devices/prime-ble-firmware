// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Nordic Uart Service ([NUS]) implementation.
//! [NUS]: https://developer.nordicsemi.com/nRF_Connect_SDK/doc/latest/nrf/libraries/bluetooth_services/services/nus.html

use crate::BT_DATA_RX;
use consts::ATT_MTU;
use defmt::{debug, error, info};
use heapless::Vec;
use nrf_softdevice::gatt_service;

#[gatt_service(uuid = "6E400001-B5A3-F393-E0A9-E50E24DCCA9E")]
pub struct Nus {
    #[characteristic(uuid = "6E400002-B5A3-F393-E0A9-E50E24DCCA9E", write_without_response)]
    rx: Vec<u8, ATT_MTU>,

    #[characteristic(uuid = "6E400003-B5A3-F393-E0A9-E50E24DCCA9E", notify)]
    tx: Vec<u8, ATT_MTU>,
}

impl Nus {
    pub(crate) fn handle(&self, event: NusEvent) {
        match event {
            NusEvent::TxCccdWrite { notifications } => {
                info!("Enable NUS: {}", notifications);
            }
            NusEvent::RxWrite(data) => {
                debug!("Received: {} bytes 0x{:02x}", data.len(), data[0]);
                if BT_DATA_RX.try_send(data).is_err() {
                    error!("Error BT_DATA_RX");
                }
            }
        }
    }

    pub(crate) fn get_handle(&self) -> u16 {
        self.tx_value_handle
    }
}
