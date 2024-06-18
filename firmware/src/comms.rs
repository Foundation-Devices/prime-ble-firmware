use crate::consts::{BT_BUFF_MAX_PKT_LEN, BT_MAX_NUM_PKT};
use crate::TX_BT_VEC;
use crate::{BT_DATA_RX, BT_STATE, BUFFERED_UART, RSSI_VALUE};
use defmt::info;
use embassy_nrf::{
    buffered_uarte::BufferedUarte,
    peripherals::{TIMER1, UARTE0},
};
use embassy_time::with_timeout;
use embassy_time::Duration;
use embedded_io_async::Write;
use heapless::Vec;
use host_protocol::{Message, MsgKind, SysStatusCommands};
use postcard::accumulator::{CobsAccumulator, FeedResult};
use postcard::to_slice_cobs;

#[embassy_executor::task]
pub async fn comms_task() {
    // Raw buffer - 32 bytes for the accumulator of cobs
    let mut raw_buf = [0u8; 32];
    // Create a cobs accumulator for data incoming
    let mut cobs_buf: CobsAccumulator<32> = CobsAccumulator::new();
    loop {
        {
            // Getting chars from Uart in a while loop
            let mut uart_in = BUFFERED_UART.lock().await;
            if let Some(uart) = uart_in.as_mut() {
                if let Ok(n) =
                    with_timeout(Duration::from_millis(500), uart.read(&mut raw_buf)).await
                {
                    // Finished reading input
                    let n = n.unwrap();
                    if n == 0 {
                        info!("overfull");
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
                                        packet_accumulator(uart, remaining).await;
                                        break 'cobs;
                                    }
                                    MsgKind::SystemStatus => sys_status_parser(&data, uart).await,
                                    MsgKind::FwUpdate => info!("Fw Update rx"),
                                    MsgKind::BtDeviceNearby => info!("Nearby rx"),
                                };

                                remaining
                            }
                        };
                    }
                }
            }
        }
        embassy_time::Timer::after_millis(1).await;
    }
}

///Accumulate packet coming from MCU to send via BT
pub async fn packet_accumulator(
    uart: &mut BufferedUarte<'static, UARTE0, TIMER1>,
    remaining: &[u8],
) {
    // Raw buffer - 32 bytes for the accumulator of cobs
    let mut vector: Vec<u8, 256> = Vec::new();
    let mut raw_buf = [0xFFu8; 32];
    //  with_timeout(timeout, fut)
    // Starting index of accumulator
    let mut idx: usize = 0;

    if remaining.len() > 0 {
        idx = remaining.len();
        let _ =vector.extend_from_slice(remaining);
    }

    // Getting chars from Uart in a while loop
    while let Ok(n) = uart.read(&mut raw_buf).await {
        // Finished reading input
        if n == 0 {
            break;
        }
        info!("Data for BT tx incoming {}", n);

        let zero_pos = raw_buf.iter().position(|&i| i == 0);
        info!("Zero pos {}", zero_pos);

        if let Some(end) = zero_pos {
            let _ =vector.extend_from_slice(&raw_buf[..end]);
            let mut buffer_tx_bt = TX_BT_VEC.lock().await;
            if buffer_tx_bt.len() < BT_MAX_NUM_PKT {
                let _ = buffer_tx_bt.push(vector);
            }
            info!("BT FINAL DATA {}", *buffer_tx_bt);
            break;
        } else {
            //Add bytes incoming
            if idx + n < BT_BUFF_MAX_PKT_LEN {
                vector.extend_from_slice(&raw_buf[..n]).unwrap();
                info!("Rx bytes : {}", vector);
                idx += n;
                info!("Buffer full at index : {}", idx);
            } else {
                info!("Ovreflow buffer - rewinding");
                idx = 0;
                break;
            }
        }
    }
}

pub async fn sys_status_parser(
    msg_recv: &Message,
    uart: &mut BufferedUarte<'static, UARTE0, TIMER1>,
) {
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
                raw_buf[0] = MsgKind::SystemStatus as u8;
                raw_buf[1] = SysStatusCommands::BTSignalStrength as u8;
                raw_buf[2] = rssi;
            }
        }
        info!("sending data");
        // Create cobs slice and send
        let cobs_tx = to_slice_cobs(&raw_buf, &mut send_buf).unwrap();
        info!("{}", cobs_tx);

        let _ = uart.write_all(cobs_tx).await;
    }
}

#[embassy_executor::task]
pub async fn send_bt_uart() {
    loop {
        let data = BT_DATA_RX.wait().await;
        {
            info!("Data rx from BT --> UART");
            // Getting chars from Uart in a while loop
            let mut uart = BUFFERED_UART.lock().await;
            if let Some(uart_tx) = uart.as_mut() {
                let _ = uart_tx.write_all(data.as_slice()).await;
                info!("{}", *data);
            }
        }
        embassy_time::Timer::after_millis(10).await;
    }
}
