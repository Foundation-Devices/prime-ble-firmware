// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Nordic Uart Service ([NUS]) implementation.
//! [NUS]: https://developer.nordicsemi.com/nRF_Connect_SDK/doc/latest/nrf/libraries/bluetooth_services/services/nus.html

use crate::{IRQ_OUT_PIN, RX_QUEUE};
use consts::APP_MTU;
use defmt::{debug, error, info};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, zerocopy_channel::Sender};
use host_protocol::Message;
use nrf_softdevice::{ble::GattValue, gatt_service};

#[gatt_service(uuid = "6E400001-B5A3-F393-E0A9-E50E24DCCA9E")]
pub struct Nus {
    #[characteristic(uuid = "6E400002-B5A3-F393-E0A9-E50E24DCCA9E", write_without_response)]
    rx: RxHack,

    #[characteristic(uuid = "6E400003-B5A3-F393-E0A9-E50E24DCCA9E", notify)]
    tx: Message,
}

struct RxHack;

impl GattValue for RxHack {
    const MIN_SIZE: usize = 0;

    const MAX_SIZE: usize = APP_MTU;

    fn from_gatt(data: &[u8]) -> Self {
        debug!("Received: {} bytes 0x{:x}", data.len(), data);
        // Notify MCU that we got something
        if let Ok(lock) = IRQ_OUT_PIN.try_lock() {
            lock.borrow_mut().as_mut().map(|pin| pin.set_low());
        }

        if let Some(buffer) = RX_QUEUE.send() {
            buffer[..data.len()].copy_from_slice(data);
            RX_QUEUE.send_done(data.len());
        } else {
            error!("Error BT_DATA_RX");
        }
        Self
    }

    fn to_gatt(&self) -> &[u8] {
        &[]
    }
}

impl Nus {
    pub(crate) fn get_handle(&self) -> u16 {
        self.tx_value_handle
    }
}
