// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Nordic Uart Service ([NUS]) implementation.
//! [NUS]: https://developer.nordicsemi.com/nRF_Connect_SDK/doc/latest/nrf/libraries/bluetooth_services/services/nus.html
//!
//! This module implements the Nordic UART Service (NUS) which provides a UART-like
//! communication channel over BLE. It consists of two characteristics:
//! - RX: For receiving data from the connected device (write without response)
//! - TX: For sending data to the connected device (notify)
//!
//! The service uses a channel-based approach to handle received data, buffering it
//! in BT_DATA_RX for processing by other parts of the application.

use crate::{BT_ADDRESS, BT_ADDRESS_MPU_TX, BT_DATA_RX, BT_STATE, BT_STATE_MPU_TX, RSSI_VALUE, RSSI_VALUE_MPU_TX};
use consts::ATT_MTU;
use defmt::info;
use heapless::Vec;
use nrf_softdevice::gatt_service;

use crate::consts;

/// UUID for the Nordic UART Service
pub(crate) const NUS_UUID: u128 = 0x6E400001_B5A3_F393_E0A9_E50E24DCCA9E;

use core::sync::atomic::Ordering;

/// Manages Bluetooth connection state and related operations
pub struct BleState;

impl BleState {
    /// Returns the current Bluetooth connection state
    #[inline]
    pub fn is_connected() -> bool {
        BT_STATE.load(Ordering::Acquire)
    }

    /// Checks if there is a pending Bluetooth state notification for the MPU
    #[inline]
    pub fn notify_bt_state() -> bool {
        BT_STATE_MPU_TX.load(Ordering::Relaxed)
    }

    /// Sets the flag to notify MPU about Bluetooth state changes
    #[inline]
    pub fn set_notify_bt_state() {
        BT_STATE_MPU_TX.store(true, Ordering::Release);
    }

    /// Clears the flag for MPU Bluetooth state notifications
    #[inline]
    pub fn clear_notify_bt_state() {
        BT_STATE_MPU_TX.store(false, Ordering::Relaxed);
    }

    /// Updates the Bluetooth connection state and triggers MPU notification
    #[inline]
    pub fn set_ble_state(state: bool) {
        BT_STATE.store(state, Ordering::Release);
        BT_STATE_MPU_TX.store(true, Ordering::Relaxed);
    }

    /// Sets the flag to notify MPU about new RSSI value
    #[inline]
    pub fn set_notify_rssi() {
        RSSI_VALUE_MPU_TX.store(true, Ordering::Relaxed)
    }

    /// Checks if there is a pending RSSI notification for the MPU
    #[inline]
    pub fn notify_rssi() -> bool {
        RSSI_VALUE_MPU_TX.load(Ordering::Relaxed)
    }

    /// Clears the flag for MPU RSSI notifications
    #[inline]
    pub fn clear_notify_rssi() {
        RSSI_VALUE_MPU_TX.store(false, Ordering::Relaxed)
    }

    /// Gets the current RSSI value
    #[inline]
    pub fn get_rssi() -> u8 {
        RSSI_VALUE.load(Ordering::Relaxed)
    }

    /// Updates the RSSI value and notifies the MPU
    #[inline]
    pub fn set_rssi(rssi: u8) {
        RSSI_VALUE.store(rssi, Ordering::Relaxed)
    }

    /// Sets the flag to notify MPU about new BLE address
    #[inline]
    pub fn set_notify_bt_address() {
        BT_ADDRESS_MPU_TX.store(true, Ordering::Release);
    }

    /// Checks if there is a pending BLE address notification for the MPU
    #[inline]
    pub fn notify_bt_address() -> bool {
        BT_ADDRESS_MPU_TX.load(Ordering::Relaxed)
    }

    /// Clears the flag for MPU BLE address notifications
    #[inline]
    pub fn clear_notify_bt_address() {
        BT_ADDRESS_MPU_TX.store(false, Ordering::Relaxed);
    }

    /// Gets the current BLE address
    #[inline]
    pub async fn get_bt_address() -> [u8; 6] {
        *BT_ADDRESS.lock().await
    }
}

/// Nordic UART Service implementation with RX and TX characteristics
#[gatt_service(uuid = "6E400001-B5A3-F393-E0A9-E50E24DCCA9E")]
pub struct Nus {
    /// RX characteristic for receiving data from connected device
    #[characteristic(uuid = "6E400002-B5A3-F393-E0A9-E50E24DCCA9E", write_without_response)]
    rx: Vec<u8, ATT_MTU>,

    /// TX characteristic for sending data to connected device
    #[characteristic(uuid = "6E400003-B5A3-F393-E0A9-E50E24DCCA9E", notify)]
    tx: Vec<u8, ATT_MTU>,
}

impl Nus {
    /// Handles NUS events like enabling notifications and receiving data
    pub(crate) fn handle(&self, event: NusEvent) {
        match event {
            // Handle enabling/disabling of TX notifications
            NusEvent::TxCccdWrite { notifications } => {
                info!("Enable UART: {}", notifications);
            }
            // Handle received data on RX characteristic
            NusEvent::RxWrite(data) => {
                // Only process data that fits within ATT_MTU and when receive buffer isn't full
                if data.len() <= ATT_MTU && !BT_DATA_RX.is_full() {
                    // Try to send data to the receive channel, log error if buffer is full
                    if BT_DATA_RX.try_send(data).is_err() {
                        info!("Error BT_DATA_RX");
                    }
                }
            }
        }
    }

    /// Returns the handle for the TX characteristic used for notifications
    pub(crate) fn get_handle(&self) -> u16 {
        self.tx_value_handle
    }
}
