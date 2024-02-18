use crate::nus::*;
use defmt::{debug, info, warn};
use nrf_softdevice::ble::gatt_server::{RegisterError, WriteOp};
use nrf_softdevice::ble::peripheral::Config;
use nrf_softdevice::ble::{gatt_server, Connection, DisconnectedError};
use nrf_softdevice::{gatt_server, Softdevice};

#[gatt_server]
pub struct Server {
    nus: Nus,
}

impl Server {
    pub(crate) async fn run(&self, conn: &Connection, config: &Config) -> DisconnectedError {
        let e = gatt_server::run(&conn, &*self, |e| self.handle_event(e)).await;
        info!("gatt_server run exited with error: {:?}", e);
        e
    }

    fn handle_event(&self, event: ServerEvent) {
        match event {
            ServerEvent::Nus(e) => self.nus.handle(e),
        }
    }
}
