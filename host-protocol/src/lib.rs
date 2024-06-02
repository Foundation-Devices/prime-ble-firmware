#![no_std]

use postcard::experimental::schema::Schema;
use serde::{Deserialize, Serialize};

pub mod sleep {
    use postcard_rpc::endpoint;

    use super::*;

    endpoint!(SleepEndpoint, Sleep, SleepDone, "done");

    #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
    pub struct Sleep {
        pub seconds: u32,
        pub micros: u32,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
    pub struct SleepDone {
        pub slept_for: Sleep,
    }
}

pub mod wire_error {
    use postcard_rpc::Key;

    use super::*;

    pub const ERROR_PATH: &str = "error";
    pub const ERROR_KEY: Key = Key::for_path::<FatalError>(ERROR_PATH);

    #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
    pub enum FatalError {
        UnknownEndpoint,
        NotEnoughSenders,
        WireFailure,
    }
}


// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

////! MPU to BLE MCU communication protocol.
////! The MPU running keyOS is the host and nRF52x BLE is the target MCU.

// #![no_std]

// use postcard::experimental::schema::Schema;
// use serde::{Deserialize, Serialize};

// /// Disables BLE and puts the MCU into low power mode until the host asserts the IRQ line.
// pub mod sleep_until_irq {
//     use postcard_rpc::endpoint;

//     use super::*;

//     endpoint!(SleepUntilIrqEndpoint, SleepUntilIrq, SleepingNow, "done");

//     #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
//     pub struct SleepUntilIrq;

//     #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
//     pub struct SleepingNow;
// }

// // TODO: status       - read status info (status maks, num NUS packets received, num. connections, etc.)
// // TODO: ble_nus_send - send a packet via BLE NUS (Nordic UART emulation over BLE)
// // TODO: ble_nus_recv - read last received packet from BLE NUS
