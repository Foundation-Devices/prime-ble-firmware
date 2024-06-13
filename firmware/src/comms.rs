use crate::TX_BT_VEC;
use crate::{BT_STATE, RSSI_VALUE};
use defmt::info;
use embassy_nrf::{
    buffered_uarte::BufferedUarte,
    peripherals::{TIMER1, UARTE0},
};
use embedded_io_async::Write;
use heapless::Vec;
use num_enum::TryFromPrimitive;
use postcard::accumulator::{CobsAccumulator, FeedResult};
use postcard::to_slice_cobs;
use serde::{Deserialize, Serialize};

const FILL_PKT_MARKER : u8 = 0xFF;
const PKT1_START : u16 = 0;
const PKT2_START : u16 = 0x100;


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
    pub msg: [u8; 32],
}

impl Message{
    pub fn new()->Self{
        Self { msg_type: MsgKind::SystemStatus, msg: [0; 32] }
    }
}

#[embassy_executor::task]
pub async fn comms_task(mut uart: BufferedUarte<'static, UARTE0, TIMER1>) {
    // Raw buffer - 32 bytes for the accumulator of cobs
    let mut raw_buf = [0u8; 32];
    // Create a cobs accumulator for data incoming
    let mut cobs_buf: CobsAccumulator<48> = CobsAccumulator::new();

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
                        MsgKind::BtData => {
                            info!("BT data rx");
                            packet_accumulator(&data , &mut uart);
                        }
                        MsgKind::SystemStatus => sys_status_parser(&data, &mut uart).await,
                        MsgKind::FwUpdate => info!("Fw Update rx"),
                        MsgKind::BtDeviceNearby => info!("Nearby rx"),
                    };

                    remaining
                }
            };
        }
    }
}


///Accumulate packet coming from MCU to send via BT
pub async fn packet_accumulator(msg_recv: &Message, uart: &mut BufferedUarte<'static, UARTE0, TIMER1>) {

     // Raw buffer - 32 bytes for the accumulator of cobs
     let mut vector : Vec<u8,256> = Vec::new();
     let mut raw_buf = [0u8; 32];

 
     // Getting chars from Uart in a while loop
     while let Ok(n) = uart.read(&mut raw_buf).await {
         // Finished reading input
         if n == 0 {
             break;
         }
         info!("Data for BT tx incoming {}", n);

         let buf = &raw_buf[..n];

         let zero_pos = raw_buf.iter().position(|&i| i == 0);

         if let Some(n) = zero_pos {
            todo!()
         }
         else {
             todo!()
         }
     };
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
pub enum SysStatusCommands {
    BtDisable,
    BtEnable,
    SystemReset,
    BTSignalStrength,
}

pub async fn sys_status_parser(msg_recv: &Message, uart: &mut BufferedUarte<'static, UARTE0, TIMER1>) {
    // Match of type of msg
    info!("parsed {}", msg_recv.msg[0]);
    // Raw buffer - 32 bytes for the accumulator of cobs
    let mut raw_buf = [0u8; 16];
    let mut send_buf = [0u8; 32];

    let cmd_as_u8: Result<SysStatusCommands, _> = msg_recv.msg[0].try_into();

    if let Ok(cmd) = cmd_as_u8 {
        match cmd {
            SysStatusCommands::BtEnable => {
                BT_STATE.signal(true);
                info!("new bt state true");
                raw_buf[0] = MsgKind::SystemStatus as u8;
                raw_buf[1] = SysStatusCommands::BtEnable as u8;
                raw_buf[2] = 0x01;

            }
            SysStatusCommands::BtDisable => {
                BT_STATE.signal(false);
                info!("new bt state false");
                raw_buf[0] = MsgKind::SystemStatus as u8;
                raw_buf[1] = SysStatusCommands::BtDisable as u8;
                raw_buf[2] = 0x00;

            }
            SysStatusCommands::SystemReset => info!("NRF RESET"),
            SysStatusCommands::BTSignalStrength => {
                let rssi = *RSSI_VALUE.lock().await;
                info!("Get RSSI {}", rssi);
                raw_buf[0]= MsgKind::SystemStatus as u8;
                raw_buf[1] = SysStatusCommands::BTSignalStrength as u8;
                raw_buf[2] = rssi;
            }
        }
        info!("sending data");
        // Create cobs slice and send
        let cobs_tx = to_slice_cobs(&raw_buf, &mut send_buf).unwrap();
        info!("{}", cobs_tx);

        uart.write_all(cobs_tx).await;
    }




}
