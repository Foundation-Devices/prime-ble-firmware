use defmt::info;
use embassy_nrf::{
    buffered_uarte::BufferedUarte,
    peripherals::{TIMER1, UARTE0},
};

use num_enum::TryFromPrimitive;
use postcard::accumulator::{CobsAccumulator, FeedResult};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum MsgKind {
    BtData = 0x01,
    SystemStatus,
    FwUpdate,
    BtDeviceNearby,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub msg_type: MsgKind,
    pub msg: [u8; 16],
}

#[embassy_executor::task]
pub async fn comms_task(mut uart: BufferedUarte<'static, UARTE0, TIMER1>) {
    // Raw buffer - 32 bytes for the accumulator of cobs
    let mut raw_buf = [0u8; 32];
    // Create a cobs accumulator for data incoming
    let mut cobs_buf: CobsAccumulator<32> = CobsAccumulator::new();

    // Getting chars from Uart in a while loop
    while let Ok(n) = uart.read(&mut raw_buf).await {
        // Finished reading input
        if n == 0 {
            break;
        }
        info!("Data incoming {}", n);

        let buf = &raw_buf[..n];
        let mut window = buf;

        'cobs: while !window.is_empty() {
            window = match cobs_buf.feed::<Message>(window) {
                FeedResult::Consumed => {
                    info!("consumed");
                    break 'cobs;
                }
                FeedResult::OverFull(new_wind) => {
                    info!("overfull");
                    new_wind
                }
                FeedResult::DeserError(new_wind) => {
                    info!("DeserError");
                    new_wind
                }
                FeedResult::Success { data, remaining } => {
                    info!("Remaining {} bytes", remaining.len());

                    match data.msg_type {
                        MsgKind::BtData => info!("BT data rx"),
                        MsgKind::SystemStatus => sys_status_parser(&data),
                        MsgKind::FwUpdate => info!("Fw Update rx"),
                        MsgKind::BtDeviceNearby => info!("Nearby rx"),
                    };

                    remaining
                }
            };
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
pub enum SysStatusCommands {
    BtEnable,
    BtDisable,
    SystemReset,
    BTSignalStrength,
}

pub fn sys_status_parser(msg_recv: &Message) {
    // Match of type of msg
    let cmd_as_u8: Result<SysStatusCommands, _> = msg_recv.msg[0].try_into();

    if let Ok(cmd) = cmd_as_u8 {
        match cmd {
            SysStatusCommands::BtEnable => info!("BT ON"), //sd_softdevice_enable(p_clock_lf_cfg, fault_handler),
            SysStatusCommands::BtDisable => info!("BT OFF"),
            SysStatusCommands::SystemReset => info!("NRF RESET"),
            SysStatusCommands::BTSignalStrength => info!("RSSI"),
        }
    }
}
