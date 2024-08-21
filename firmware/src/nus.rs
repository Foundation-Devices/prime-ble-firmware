// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Nordic Uart Service ([NUS]) implementation.
//! [NUS]: https://developer.nordicsemi.com/nRF_Connect_SDK/doc/latest/nrf/libraries/bluetooth_services/services/nus.html

use crate::BT_DATA_RX;
use consts::ATT_MTU;
use defmt::info;
use heapless::Vec;
use nrf_softdevice::gatt_service;

use crate::consts;

pub(crate) const NUS_UUID: u128 = 0x6E400001_B5A3_F393_E0A9_E50E24DCCA9E;

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
                info!("Enable UART: {}", notifications);
            }
            NusEvent::RxWrite(data) => {
                // If we receive something bigger for some reasons discard it
                if data.len() <= ATT_MTU && !BT_DATA_RX.is_full() {
                    // info!("Received: {} bytes {:?}", data.len(), data);
                    if BT_DATA_RX.try_send(data).is_err() {
                        info!("Error BT_DATA_RX");
                    }
                }
            }
        }
    }

    pub(crate) fn get_handle(&self) -> u16 {
        self.tx_value_handle
    }
}
