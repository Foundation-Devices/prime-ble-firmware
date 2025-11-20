// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Nordic Uart Service ([NUS]) implementation.
//! [NUS]: https://developer.nordicsemi.com/nRF_Connect_SDK/doc/latest/nrf/libraries/bluetooth_services/services/nus.html

use crate::{BT_DATA_RX, IRQ_OUT_PIN};
use defmt::{debug, error, info};
use host_protocol::Message;
use nrf_softdevice::gatt_service;

#[gatt_service(uuid = "6E400001-B5A3-F393-E0A9-E50E24DCCA9E")]
pub struct Nus {
    #[characteristic(uuid = "6E400002-B5A3-F393-E0A9-E50E24DCCA9E", write_without_response)]
    rx: Message,

    #[characteristic(uuid = "6E400003-B5A3-F393-E0A9-E50E24DCCA9E", notify)]
    tx: Message,
}

impl Nus {
    pub(crate) fn handle(&self, event: NusEvent) {
        match event {
            NusEvent::TxCccdWrite { notifications } => {
                info!("Enable NUS: {}", notifications);
            }
            NusEvent::RxWrite(data) => {
                debug!("Received: {} bytes 0x{:x}", data.len(), data);
                if BT_DATA_RX.try_send(data).is_err() {
                    error!("Error BT_DATA_RX");
                }
                // Notify MCU that we got something
                if let Ok(mut lock) = IRQ_OUT_PIN.try_lock() {
                    lock.as_mut().map(|pin| pin.set_low());
                }
            }
        }
    }

    pub(crate) fn get_handle(&self) -> u16 {
        self.tx_value_handle
    }
}
