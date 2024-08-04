use crate::consts::{BT_MAX_NUM_PKT, MTU};
use crate::{BT_DATA_RX, BT_STATE, RSSI_TX, RSSI_VALUE};
use crate::{IRQ_OUT_PIN, TX_BT_VEC};
use defmt::info;
use embassy_nrf::buffered_uarte::{BufferedUarteRx, BufferedUarteTx};
use embassy_nrf::peripherals::{TIMER1, UARTE0};
use embassy_time::Instant;
use embedded_io_async::Write;
use heapless::Vec;
use host_protocol::{Bluetooth, HostProtocolMessage, COBS_MAX_MSG_SIZE};
use postcard::accumulator::{CobsAccumulator, FeedResult};
use postcard::to_slice_cobs;

#[embassy_executor::task]
pub async fn comms_task(mut rx: BufferedUarteRx<'static, 'static, UARTE0, TIMER1>) {
    // Raw buffer - 32 bytes for the accumulator of cobs
    let mut raw_buf = [0u8; 128];

    // let mut counter = 0;
    // let mut data_rx = 0;
    // Create a cobs accumulator for data incoming
    let mut cobs_buf: CobsAccumulator<COBS_MAX_MSG_SIZE> = CobsAccumulator::new();
    loop {
        {
            // Getting chars from Uart in a while loop
            // let mut uart_in = BUFFERED_UART.lock().await;
            // if let Some(uart) = uart_in.as_mut() {
            if let Ok(n) = rx.read(&mut raw_buf).await {
                // Finished reading input
                // let n = n.unwrap();
                if n == 0 {
                    info!("overfull");
                    break;
                }
                // info!("Data incoming {} bytes", n);

                let buf = &raw_buf[..n];
                let mut window = buf;

                'cobs: while !window.is_empty() {
                    window = match cobs_buf.feed_ref::<HostProtocolMessage>(window) {
                        FeedResult::Consumed => {
                            // info!("consumed");
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
                                    // match bluetooth_msg {
                                    //     Bluetooth::SendData(data) =>{
                                    //     counter += 1;
                                    //     data_rx += data.len();
                                    //     info!("Packet recv {} - data recv {}",counter,data_rx);
                                    //     },
                                    //     _ => {}
                                    // }
                                    bluetooth_handler(bluetooth_msg).await
                                }
                                HostProtocolMessage::Bootloader(_) => (), // no-op, handled in the bootloader
                                HostProtocolMessage::Reset => {
                                    info!("Resetting");
                                    drop(rx);
                                    cortex_m::peripheral::SCB::sys_reset();
                                }
                            };

                            remaining
                        }
                    };
                }
            }
        }
    }
}

async fn bluetooth_handler(msg: Bluetooth<'_>) {
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
            // Send value to channel
            let _ = RSSI_TX.try_send(rssi);
        }
        Bluetooth::SignalStrength(_) => (), // no-op, host-side packet
        Bluetooth::SendData(data) => {
            // info!("Sending BLE data: {:?}", data);
            // Error if data length is greater than max MTU size
            if data.len() <= MTU {
                let mut buffer_tx_bt = TX_BT_VEC.lock().await;
                // info!("Buffer to BT len {:?}", buffer_tx_bt.len());
                if buffer_tx_bt.len() < BT_MAX_NUM_PKT {
                    let _ = buffer_tx_bt.push(Vec::from_slice(data).unwrap());
                }
            }
        }
        Bluetooth::ReceivedData(_) => {} // no-op, host-side packet
    }
}

/// Sends the data received from the BLE NUS as `host-protocol` encoded data message.
#[embassy_executor::task]
pub async fn send_bt_uart(mut uart_tx: BufferedUarteTx<'static, 'static, UARTE0, TIMER1>) {
    let mut send_buf = [0u8; 1024];
    let mut rx_buf: Vec<u8, 1024> = Vec::new();

    let mut data_count: u32 = 0;
    let mut total_data: u32 = 0;

    // Get time to discard packet counter in case of long time with no rx
    // this shit must be more secure just a quick solution
    let mut now = Instant::now();

    loop {
        if let Ok(rssi) = RSSI_TX.try_receive() {
            send_buf.fill(0); // Clear the buffer from any previous data

            info!("Sending back RSSI: {}", rssi);

            let msg = HostProtocolMessage::Bluetooth(Bluetooth::SignalStrength(rssi));
            let cobs_tx = to_slice_cobs(&msg, &mut send_buf).unwrap();
            info!("{}", cobs_tx);

            let _ = uart_tx.write_all(cobs_tx).await;
            let _ = uart_tx.flush().await;
            assert_out_irq().await; // Ask the MP
        }

        // Try receive from BT sender channel
        let cobs = if let Ok(data) = BT_DATA_RX.try_receive() {
            now = Instant::now();

            if data_count == 0 {
                // Get length
                let (len, data) = data.split_at(4);
                total_data = u32::from_ne_bytes(len.try_into().unwrap());
                data_count += data.len() as u32;
                let _ = rx_buf.extend_from_slice(data);
                info!("total data: {} - data count {} - rx buf {}", total_data, data_count, rx_buf)
            } else {
                data_count += data.len() as u32;
                if data_count <= 1024 {
                    rx_buf.extend(data);
                    info!("total data: {} - data count {} - rx buf {}", total_data, data_count, rx_buf)
                }
            };
            if data_count == total_data {
                let msg = HostProtocolMessage::Bluetooth(Bluetooth::ReceivedData(rx_buf.as_slice()));
                send_buf.fill(0); // Clear the buffer from any previous data
                Some(to_slice_cobs(&msg, &mut send_buf).expect("to_slice_cobs"))
            } else {
                None
            }
        } else {
            None
        };

        {
            // If data is present from BT send to serial with Cobs format
            if let Some(cobs_tx) = cobs {
                info!("Data rx from BT --> UART");
                // Getting chars from Uart in a while loop
                let _ = uart_tx.write_all(cobs_tx).await;
                let _ = uart_tx.flush().await;

                info!("{}", *cobs_tx);
                assert_out_irq().await; // Ask the MPU to process a new packet we just sent

                total_data = 0;
                data_count = 0;
                rx_buf.clear();
            }
        }

        // If we don't receive packet for more than one second just clear all data counter for safety
        if now.elapsed().as_millis() > 1500 || data_count > 1024 {
            total_data = 0;
            data_count = 0;
            rx_buf.clear();
        }

        embassy_time::Timer::after_micros(200).await;
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
