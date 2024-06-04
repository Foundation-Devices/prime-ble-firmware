use embassy_nrf::{buffered_uarte::BufferedUarte, peripherals::{TIMER0, UARTE0}};
use serde::{Deserialize, Serialize};
use postcard::accumulator::{FeedResult, CobsAccumulator};
use nrf_softdevice_s112::{sd_ble_enable, sd_softdevice_disable, sd_softdevice_enable};
use defmt::info;
use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum MsgKind {
    BtData = 0x01,
    SystemStatus,
    FwUpdate,
    BtDeviceNearby,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub msg_type : MsgKind,
    pub msg  :  [u8; 32],
}


#[embassy_executor::task]
pub async fn comms_task( mut uart: BufferedUarte<'static, UARTE0, TIMER0>) {
    
    // Raw buffer - 32 bytes for the accumulator of cobs
    let mut raw_buf = [0u8; 256];
    // Create a cobs accumulator for data incoming
    let mut cobs_buf: CobsAccumulator<256> = CobsAccumulator::new();

    // Getting chars from Uart in a while loop
    while let Ok(n) = uart.read(&mut raw_buf).await{
        // Finished reading input
        if n == 0 {
            break;
        }

        //let mut output : Message = None;

        let buf = &raw_buf[..n];
        let mut window = buf;

        'cobs: while !window.is_empty() {
            window = match cobs_buf.feed::<Message>(window) {
                FeedResult::Consumed => break 'cobs,
                FeedResult::OverFull(new_wind) => new_wind,
                FeedResult::DeserError(new_wind) => new_wind,
                FeedResult::Success { data, remaining } => {

                    info!("Remaining {} bytes",remaining.len());

                    match data.msg_type {
                        MsgKind::BtData => todo!(),
                        MsgKind::SystemStatus => sys_status_parser(&data),
                        MsgKind::FwUpdate => todo!(),
                        MsgKind::BtDeviceNearby => todo!(),
                    }

                }
            };
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
pub enum SysStatusCommands {
    BtEnable = 1,
    BtDisable,
    SystemReset,
    BTSignalStrength,
}

pub fn sys_status_parser( msg_recv: &Message ) {
    // Match of type of msg
    let cmd : Result<SysStatusCommands, _> = msg_recv.msg[0].try_into();

    match cmd.unwrap()  {
        SysStatusCommands::BtEnable => todo!(),//sd_softdevice_enable(p_clock_lf_cfg, fault_handler),
        SysStatusCommands::BtDisable => todo!(),//sd_softdevice_disable(),
        SysStatusCommands::SystemReset => todo!(),
        SysStatusCommands::BTSignalStrength => todo!(),
    }
}

