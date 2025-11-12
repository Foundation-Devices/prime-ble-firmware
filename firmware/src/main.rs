// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]
#![no_main]

mod comms;
mod nus;
mod server;

use core::cell::RefCell;
use core::pin::pin;
use core::sync::atomic::{AtomicBool, AtomicI8, AtomicU8};
#[cfg(feature = "debug")]
use defmt_rtt as _;
// global logger
use embassy_nrf as _;
use embassy_sync::rwlock::RwLock;
use host_protocol::Message;
// time driver
use panic_probe as _;

use comms::comms_task;
use defmt::{info, *};
use embassy_executor::Spawner;
use embassy_nrf::bind_interrupts;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::interrupt::{self, InterruptExt};
use embassy_nrf::{
    peripherals::SPI0,
    spis::{self, Spis},
};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use nrf52805_pac::FICR;
use nrf_softdevice::ble::{get_address, security::SecurityHandler, Connection, SecurityMode};
use nrf_softdevice::Softdevice;
use server::{initialize_sd, run_bluetooth, Server};

bind_interrupts!(struct Irqs {
    SPIM0_SPIS0_SPI0 => spis::InterruptHandler<SPI0>;
});

#[cfg(not(feature = "debug"))]
mod dummy_logging {
    #[defmt::global_logger]
    struct Logger;

    unsafe impl defmt::Logger for Logger {
        fn acquire() {}

        unsafe fn flush() {}

        unsafe fn release() {}

        unsafe fn write(_bytes: &[u8]) {}
    }
}

/// Maximum number of BLE packets that can be buffered.
/// This limits memory usage while ensuring reliable data transfer.
pub const BT_MAX_NUM_PKT: usize = 16;

// Signal for BT state
static BT_STATE: AtomicBool = AtomicBool::new(false);
static BT_ADV_CHAN: AtomicU8 = AtomicU8::new(0);
static BT_DATA_RX: Channel<ThreadModeRawMutex, Message, BT_MAX_NUM_PKT> = Channel::new();
static TX_PWR_VALUE: AtomicI8 = AtomicI8::new(0i8);

static CONNECTION: RwLock<ThreadModeRawMutex, Option<Connection>> = RwLock::new(None);
static PAIRED: AtomicBool = AtomicBool::new(false);

/// nRF -> MPU IRQ output pin
static IRQ_OUT_PIN: Mutex<ThreadModeRawMutex, RefCell<Option<Output>>> = Mutex::new(RefCell::new(None));

struct BleSecurityHandler;

static BLE_SECURITY_HANDLER: BleSecurityHandler = BleSecurityHandler;

impl SecurityHandler for BleSecurityHandler {
    fn on_security_update(&self, _conn: &Connection, security_mode: SecurityMode) {
        let is_paired = !matches!(security_mode, SecurityMode::Open | SecurityMode::NoAccess);
        PAIRED.store(is_paired, core::sync::atomic::Ordering::Relaxed);
        if is_paired {
            info!("BLE connection paired: {:?}", security_mode);
        } else {
            info!("BLE connection unpaired: {:?}", security_mode);
        }
    }
}

/// Check if the current BLE connection is paired
pub fn is_paired() -> bool {
    PAIRED.load(core::sync::atomic::Ordering::Relaxed)
}

/// Get the current security mode of the connection (if connected)
pub async fn get_connection_security_mode() -> Option<SecurityMode> {
    let conn_lock = CONNECTION.read().await;
    conn_lock.as_ref().map(|conn| conn.security_mode())
}

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) -> ! {
    info!("SD is running");
    sd.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut conf = embassy_nrf::config::Config::default();
    // This caused bad behaviour at reset - will check if i did something wrong
    // conf.dcdc = embassy_nrf::config::DcdcConfig { reg1: true };
    conf.hfclk_source = embassy_nrf::config::HfclkSource::ExternalXtal;
    conf.lfclk_source = embassy_nrf::config::LfclkSource::ExternalXtal;

    conf.gpiote_interrupt_priority = interrupt::Priority::P2;
    conf.time_interrupt_priority = interrupt::Priority::P2;

    let p = embassy_nrf::init(conf);

    let spi = {
        // Configure SPI
        let mut config_spi = spis::Config::default();
        config_spi.orc = 0x51; // to detect padding
        Spis::new(p.SPI0, Irqs, p.P0_18, p.P0_16, p.P0_14, p.P0_12, config_spi)
    };

    // Configure the OUT IRQ pin
    {
        IRQ_OUT_PIN
            .lock()
            .await
            .borrow_mut()
            .replace(Output::new(p.P0_20, Level::High, OutputDrive::Standard));
    }

    // set priority to avoid collisions with softdevice
    interrupt::SPIM0_SPIS0_SPI0.set_priority(interrupt::Priority::P3);

    let sd = initialize_sd();

    let server = unwrap!(Server::new(sd), "Creating the softdevice failed");
    unwrap!(spawner.spawn(softdevice_task(sd)), "Spawning the softdevice failed");

    // Get Bt device address
    let mut address = get_address(sd).bytes();
    address.reverse();
    info!("Address : {=[u8;6]:#X}", address);

    let device_id = unsafe {
        let ficr = &*FICR::ptr();
        let device_id_low = ficr.deviceid[0].read().bits();
        let device_id_high = ficr.deviceid[1].read().bits();
        let device_id = (device_id_high as u64) << 32 | (device_id_low as u64);
        info!("Device ID : {:08x}", device_id);
        device_id.to_le_bytes()
    };
    // Comm task
    let comms = comms_task(
        spi,
        comms::CommsContext {
            address,
            device_id,
            server: &server,
        },
    );
    let ble = run_bluetooth(sd, &server);
    info!("Init tasks");

    futures::future::select(pin!(comms), pin!(ble)).await;
}
