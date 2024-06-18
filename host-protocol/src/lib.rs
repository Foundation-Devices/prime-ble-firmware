// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! MPU to BLE MCU communication protocol.
//! The MPU running keyOS is the host and nRF52x BLE is the target MCU.
//! WIll move here simpler struct for postcard messages!!

#![no_std]
use num_enum::TryFromPrimitive;
use serde::{Deserialize, Serialize};

/// Command kinds ( TBD )
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum MsgKind {
    BtData = 0x01,
    SystemStatus,
    FwUpdate,
    BtDeviceNearby,
}

/// Command for specific actions requested to Bluetooth MCU
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
pub enum SysStatusCommands {
    BtDisable,
    BtEnable,
    SystemReset,
    BTSignalStrength,
}

///Generic format data to communicate between MCUs
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub msg_type: MsgKind,
    pub msg: [u8; 16],
}

// TODO: status       - read status info (status maks, num NUS packets received, num. connections, etc.)
// TODO: ble_nus_send - send a packet via BLE NUS (Nordic UART emulation over BLE)
// TODO: ble_nus_recv - read last received packet from BLE NUS
