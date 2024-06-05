// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! MPU to BLE MCU communication protocol.
//! The MPU running keyOS is the host and nRF52x BLE is the target MCU.
//! WIll move here simpler struct for postcard messages!!

#![no_std]

use postcard::experimental::schema::Schema;
use serde::{Deserialize, Serialize};

/// Disables BLE and puts the MCU into low power mode until the host asserts the IRQ line.
pub mod sleep_until_irq {
    use postcard_rpc::endpoint;

    use super::*;

    endpoint!(SleepUntilIrqEndpoint, SleepUntilIrq, SleepingNow, "done");

    #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
    pub struct SleepUntilIrq;

    #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
    pub struct SleepingNow;
}

// TODO: status       - read status info (status maks, num NUS packets received, num. connections, etc.)
// TODO: ble_nus_send - send a packet via BLE NUS (Nordic UART emulation over BLE)
// TODO: ble_nus_recv - read last received packet from BLE NUS
