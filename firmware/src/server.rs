// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::nus::*;
use defmt::info;
use nrf_softdevice::ble::peripheral::Config;
use nrf_softdevice::ble::{gatt_server, Connection, DisconnectedError};
use nrf_softdevice::gatt_server;

#[gatt_server]
pub struct Server {
    nus: Nus,
}

impl Server {
    pub(crate) async fn run(&self, conn: &Connection, _config: &Config) -> DisconnectedError {
        let _ = conn.start_rssi();
        let e = gatt_server::run(conn, self, |e| self.handle_event(e)).await;
        info!("gatt_server run exited with error: {:?}", e);
        e
    }

    fn handle_event(&self, event: ServerEvent) {
        match event {
            ServerEvent::Nus(e) => self.nus.handle(e),
        }
    }
}
