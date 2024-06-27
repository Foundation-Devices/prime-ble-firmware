use crate::consts::BT_MAX_NUM_PKT;
use crate::{BT_DATA_RX, BT_STATE, BUFFERED_UART, RSSI_VALUE};
use crate::{IRQ_OUT_PIN, TX_BT_VEC};
use defmt::info;
use embassy_nrf::{
    buffered_uarte::BufferedUarte,
    peripherals::{TIMER1, UARTE0},
};
use embassy_time::with_timeout;
use embassy_time::Duration;
use embedded_io_async::Write;
use heapless::Vec;
use host_protocol::{Bluetooth, HostProtocolMessage, COBS_MAX_MSG_SIZE};
use postcard::accumulator::{CobsAccumulator, FeedResult};
use postcard::to_slice_cobs;

#[embassy_executor::task]
pub async fn comms_task() {
    // Raw buffer - 32 bytes for the accumulator of cobs
    let mut raw_buf = [0u8; 32];
    // Create a cobs accumulator for data incoming
    let mut cobs_buf: CobsAccumulator<COBS_MAX_MSG_SIZE> = CobsAccumulator::new();
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
                    info!("Data incoming {} bytes", n);

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
                                        info!("Received HostProtocolMessage::Bluetooth");
                                        bluetooth_handler(uart, bluetooth_msg).await
                                    }
                                    HostProtocolMessage::Bootloader(_) => (), // no-op, handled in the bootloader
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
            let _ = uart.flush().await;
            assert_out_irq().await; // Ask the MPU to process a new packet we just sent
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

/// Sends the data received from the BLE NUS as `host-protocol` encoded data message.
#[embassy_executor::task]
pub async fn send_bt_uart() {
    let mut send_buf = [0u8; COBS_MAX_MSG_SIZE];

    loop {
        let data = BT_DATA_RX.wait().await;
        let msg = HostProtocolMessage::Bluetooth(Bluetooth::ReceivedData(data.as_slice()));

        send_buf.fill(0); // Clear the buffer from any previous data
        let cobs_tx = to_slice_cobs(&msg, &mut send_buf).expect("to_slice_cobs");

        {
            info!("Data rx from BT --> UART");
            // Getting chars from Uart in a while loop
            let mut uart = BUFFERED_UART.lock().await;
            if let Some(uart_tx) = uart.as_mut() {
                let _ = uart_tx.write_all(cobs_tx).await;
                let _ = uart_tx.flush().await;

                info!("{}", *cobs_tx);
                assert_out_irq().await; // Ask the MPU to process a new packet we just sent
            }
        }
        embassy_time::Timer::after_millis(10).await;
    }
}

/// Sends a single pulse on the nRF -> MPU IRQ line, signaling the MPU to process the data.
async fn assert_out_irq() {
    let irq_out = IRQ_OUT_PIN.lock().await;

    {
        let mut button = irq_out.borrow_mut();
        // The pin should be HIGH by default, and we need a falling edge, so put it in HIGH just in case
        button.as_mut().unwrap().set_high();

        // Send the pulse
        button.as_mut().unwrap().set_low();
        button.as_mut().unwrap().set_high();
    }
}
