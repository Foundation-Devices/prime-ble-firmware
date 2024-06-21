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
use host_protocol::{Bluetooth, Bootloader, HostProtocolMessage};
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
                        window = match cobs_buf.feed_ref::<HostProtocolMessage>(window) {
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

                                match data {
                                    HostProtocolMessage::Bluetooth(bluetooth_msg) => {
                                        bluetooth_handler(uart, bluetooth_msg).await
                                    }
                                    HostProtocolMessage::Bootloader(bootloader_msg) => (), // no-op, handled in the bootloader
                                    HostProtocolMessage::Reset => {
                                        info!("Resetting");
                                        // TODO: reset
                                    }
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

async fn bluetooth_handler(uart: &mut BufferedUarte<'static, UARTE0, TIMER1>, msg: Bluetooth<'_>) {
    match msg {
        Bluetooth::Enable => {
            info!("Bluetooth enabled");
            BT_STATE.signal(true);
        }
        Bluetooth::Disable => {
            info!("Bluetooth disabled");
            BT_STATE.signal(false);
        }
        Bluetooth::GetSignalStrength => {
            info!("Get signal strength");
            let rssi = *RSSI_VALUE.lock().await;
            info!("RSSI: {}", rssi);

            let mut response_buf = [0u8; 16];
            let msg = HostProtocolMessage::Bluetooth(Bluetooth::SignalStrength(rssi));
            let cobs_tx = to_slice_cobs(&msg, &mut response_buf).unwrap();
            info!("{}", cobs_tx);

            let _ = uart.write_all(cobs_tx).await;
        }
        Bluetooth::SignalStrength(_) => (), // no-op, host-side packet
        Bluetooth::SendData(data) => {
            info!("Sending BLE data: {:?}", data);
            let mut buffer_tx_bt = TX_BT_VEC.lock().await;
            if buffer_tx_bt.len() < BT_MAX_NUM_PKT {
                let _ = buffer_tx_bt.push(Vec::from_slice(data).unwrap());
            }
        }
        Bluetooth::ReceivedData(_) => {} // no-op, host-side packet
    }
}

/*
pub async fn sys_status_parser(
    msg_recv: &HostProtocolMessage<'_>,
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
}*/

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
