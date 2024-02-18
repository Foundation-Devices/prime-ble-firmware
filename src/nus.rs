//! Nordic Uart Service ([NUS]) implementation.
//! [NUS]: https://developer.nordicsemi.com/nRF_Connect_SDK/doc/latest/nrf/libraries/bluetooth_services/services/nus.html

use consts::ATT_MTU;
use defmt::{debug, info, warn};
use heapless::Vec;
use nrf_softdevice::ble::Connection;
use nrf_softdevice::gatt_service;

use crate::consts;

pub(crate) const NUS_UUID: u128 = 0x6E400001_B5A3_F393_E0A9_E50E24DCCA9E;

#[gatt_service(uuid = "6E400001-B5A3-F393-E0A9-E50E24DCCA9E")]
pub struct Nus {
    #[characteristic(uuid = "6E400002-B5A3-F393-E0A9-E50E24DCCA9E", write)]
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
                debug!("Received: {} bytes {:?}", data.len(), data);
            }
            _ => warn!("Unhandled event"),
        }
    }
}
