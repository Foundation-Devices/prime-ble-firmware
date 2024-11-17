// Standard imports for BLE communication and cryptographic operations
use crate::consts::{BT_MAX_NUM_PKT, MTU, UICR_SECRET_SIZE, UICR_SECRET_START};
use crate::{BT_ADDRESS, BT_DATA_TX, IRQ_OUT_PIN};
use crate::{BT_DATA_RX, BT_STATE, RSSI_VALUE};
use defmt::info;
use embassy_nrf::buffered_uarte::BufferedUarte;
use embassy_nrf::peripherals::{TIMER1, UARTE0};
use embassy_time::Instant;
use embedded_io_async::Write;
use heapless::Vec;
use hmac::{Hmac, Mac};
use host_protocol::{Bluetooth, HostProtocolMessage, State, COBS_MAX_MSG_SIZE};
use postcard::accumulator::{CobsAccumulator, FeedResult};
use postcard::to_slice_cobs;
use sha2::Sha256 as ShaChallenge;

// #[inline(never)]
// pub async fn send_to_uart<'a>(msg: HostProtocolMessage<'_>, tx: &'a mut BufferedUarteTx<'a, UARTE0>) {
//     let cobs_tx = to_slice_cobs(&msg, &mut send_buf).unwrap();
//     let _ = tx.write_all(cobs_tx).await;
//     let _ = tx.flush().await;
//     assert_out_irq().await;
// }

/// Helper function to signal the MPU via GPIO
/// Sends a falling edge pulse on the IRQ line
async fn assert_out_irq() {
    let irq_out = IRQ_OUT_PIN.lock().await;

    {
        let mut button = irq_out.borrow_mut();
        // Ensure pin starts HIGH
        button.as_mut().unwrap().set_high();

        // Generate falling edge pulse
        button.as_mut().unwrap().set_low();
        button.as_mut().unwrap().set_high();
    }
}


/// Main communication task that handles incoming UART messages from the MPU
/// Decodes COBS-encoded messages and routes them to appropriate handlers
#[embassy_executor::task]
pub async fn comms_task(uart: BufferedUarte<'static, UARTE0, TIMER1>) {
    let mut send_buf = [0u8; COBS_MAX_MSG_SIZE];

    // Split UART into RX and TX
    let (mut rx, mut tx) = uart.split();

    // Buffer for raw incoming UART data
    let mut raw_buf = [0u8; 64];

    // COBS accumulator for decoding incoming messages
    let mut cobs_buf: CobsAccumulator<COBS_MAX_MSG_SIZE> = CobsAccumulator::new();
    loop {
        {
            // Check for new BLE data to send to MPU
            if let Ok(data) = BT_DATA_RX.try_receive() {
                send_buf.fill(0); // Clear the buffer from any previous data
                let msg = HostProtocolMessage::Bluetooth(Bluetooth::ReceivedData(data.as_slice()));
                let cobs_tx = to_slice_cobs(&msg, &mut send_buf).unwrap();
                let _ = tx.write_all(cobs_tx).await;
                let _ = tx.flush().await;
                assert_out_irq().await;
                // Try to send another packet if there is more data to send
                if !BT_DATA_RX.is_empty() {
                    continue;
                }
            }


            // Read data from UART
            if let Ok(n) = &rx.read(&mut raw_buf).await {
                // Clear the send buffer
                send_buf.fill(0);

                // Exit if no data received
                if *n == 0 {
                    break;
                }

                let buf = &raw_buf[..*n];
                let mut window = buf;

                // Process all complete COBS messages in the buffer
                'cobs: while !window.is_empty() {
                    window = match cobs_buf.feed_ref::<HostProtocolMessage>(window) {
                        FeedResult::Consumed => {
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
                            // Route message to appropriate handler based on type
                            match data {
                                HostProtocolMessage::Bluetooth(bluetooth_msg) => {
                                    info!("Received HostProtocolMessage::Bluetooth");
                                    let msg = bluetooth_handler(bluetooth_msg).await;
                                    if let Some(msg) = msg {
                                        if let Ok(cobs_tx) = to_slice_cobs(&msg, &mut send_buf) {
                                            let _ = tx.write_all(cobs_tx).await;
                                            let _ = tx.flush().await;
                                            assert_out_irq().await;
                                        }
                                    }
                                }
                                HostProtocolMessage::Bootloader(_) => (), // Handled in bootloader
                                HostProtocolMessage::Reset => {
                                    cortex_m::peripheral::SCB::sys_reset();
                                }
                                HostProtocolMessage::ChallengeRequest { nonce } => {
                                    let msg = hmac_challenge_response(nonce);
                                    if let Ok(cobs_tx) = to_slice_cobs(&msg, &mut send_buf) {
                                        let _ = tx.write_all(cobs_tx).await;
                                        let _ = tx.flush().await;
                                        assert_out_irq().await;
                                    }
                                }
                                HostProtocolMessage::GetState => {
                                    let msg = match BT_STATE.load(core::sync::atomic::Ordering::Relaxed) {
                                        true => HostProtocolMessage::AckState(State::Enabled),
                                        false => HostProtocolMessage::AckState(State::Disabled),
                                    };
                                    if let Ok(cobs_tx) = to_slice_cobs(&msg, &mut send_buf) {
                                        let _ = tx.write_all(cobs_tx).await;
                                        let _ = tx.flush().await;
                                        assert_out_irq().await;
                                    }
                                }
                                _ => (),
                            };
                            remaining
                        }
                    };
                }
            }
        }
    }
}

/// Handles Bluetooth-specific messages received from the MPU
async fn bluetooth_handler(msg: Bluetooth<'_>) -> Option<HostProtocolMessage<'_>> {
    match msg {
        Bluetooth::Enable => {
            info!("Bluetooth enabled");
            BT_STATE.store(true, core::sync::atomic::Ordering::Relaxed);
            return None;
        }
        Bluetooth::Disable => {
            info!("Bluetooth disabled");
            BT_STATE.store(false, core::sync::atomic::Ordering::Relaxed);
            return None;
        }
        Bluetooth::GetSignalStrength => {
            let msg = HostProtocolMessage::Bluetooth(Bluetooth::SignalStrength(RSSI_VALUE.load(core::sync::atomic::Ordering::Relaxed)));
            return Some(msg);
        }
        Bluetooth::GetFirmwareVersion => {
            let version = env!("CARGO_PKG_VERSION");
            let msg = HostProtocolMessage::Bluetooth(Bluetooth::AckFirmwareVersion { version });
            return Some(msg);
        }
        Bluetooth::SignalStrength(_) => {
            return None;
        }
        Bluetooth::SendData(data) => {
            // Only accept data packets within MTU size limit
            if data.len() <= MTU {
                let mut buffer_tx_bt = BT_DATA_TX.lock().await;
                if buffer_tx_bt.len() < BT_MAX_NUM_PKT {
                    let _ = buffer_tx_bt.push(Vec::from_slice(data).unwrap());
                }
            }
            return None;
        }
        Bluetooth::GetBtAddress => {
            let msg = HostProtocolMessage::Bluetooth(Bluetooth::AckBtAaddress { bt_address: *BT_ADDRESS.lock().await });
            return Some(msg);
        }
        Bluetooth::ReceivedData(_) => {
            return None;
        }
        Bluetooth::AckFirmwareVersion { .. } => {
            return None;
        }
        Bluetooth::AckBtAaddress { .. } => {
            return None;
        }
    }
}

/// Handles HMAC challenge-response authentication
fn hmac_challenge_response(nonce: u64) -> HostProtocolMessage<'static> {
    type HmacSha256 = Hmac<ShaChallenge>;
    // Get device secret from UICR memory
    let secret_as_slice = unsafe { core::slice::from_raw_parts(UICR_SECRET_START as *const u8, UICR_SECRET_SIZE as usize) };

    // Calculate HMAC response
    let result = if let Ok(mut mac) = HmacSha256::new_from_slice(secret_as_slice) {
        mac.update(&nonce.to_be_bytes());
        let result = mac.finalize().into_bytes();
            info!("{=[u8;32]:#X}", result.into());
            HostProtocolMessage::ChallengeResult { result: result.into() }
        } else {
        HostProtocolMessage::ChallengeResult { result: [0xFF; 32] }
    };
    result
}

// #[embassy_executor::task]
// pub async fn send_bt_uart_no_cobs(uart_tx: &'static mut BufferedUarteTx<'static, UARTE0>) {
//     let mut send_buf = [0u8; COBS_MAX_MSG_SIZE];
//     let mut data_counter: u64 = 0;
//     let mut pkt_counter: u64 = 0;
//     let mut rx_packet = false;
//     let mut timer_pkt: Instant = Instant::now();
//     let mut timer_tot: Instant = Instant::now();

//     loop {
//         // Handle Bluetooth state updates
//         if BT_STATE_MPU_TX.load(core::sync::atomic::Ordering::Relaxed) {
//             send_buf.fill(0); // Clear the buffer from any previous data

//             info!(
//                 "Sending back BT state: {}",
//                 BT_STATE_MPU_TX.load(core::sync::atomic::Ordering::Relaxed)
//             );

//             let msg = match BT_STATE.load(core::sync::atomic::Ordering::Relaxed) {
//                 true => HostProtocolMessage::AckState(State::Enabled),
//                 false => HostProtocolMessage::AckState(State::Disabled),
//             };

//             BT_STATE_MPU_TX.store(true, core::sync::atomic::Ordering::Relaxed);
//             let cobs_tx = to_slice_cobs(&msg, &mut send_buf).unwrap();
//             let _ = uart_tx.write_all(cobs_tx).await;
//             let _ = uart_tx.flush().await;
//             assert_out_irq().await;
//         }

//         // Handle HMAC challenge-response
//         if CHALLENGE_REQUEST.load(core::sync::atomic::Ordering::Relaxed) {
//             CHALLENGE_REQUEST.store(false, core::sync::atomic::Ordering::Relaxed);

//             type HmacSha256 = Hmac<ShaChallenge>;

//             // SAFETY: UICR_SECRET_START points to valid read-only memory containing the secret
//             let secret_as_slice = unsafe { core::slice::from_raw_parts(UICR_SECRET_START as *const u8, UICR_SECRET_SIZE as usize) };
//             let nonce = *CHALLENGE_NONCE.lock().await;

//             info!("Nonce: {}", nonce);
//             let result = if let Ok(mut mac) = HmacSha256::new_from_slice(secret_as_slice) {
//                 mac.update(&nonce.to_be_bytes());
//                 let result = mac.finalize().into_bytes();
//                 info!("{=[u8;32]:#X}", result.into());
//                 HostProtocolMessage::ChallengeResult { result: result.into() }
//             } else {
//                 HostProtocolMessage::ChallengeResult { result: [0xFF; 32] }
//             };

//             let cobs_tx = to_slice_cobs(&result, &mut send_buf).unwrap();
//             let _ = &uart_tx.write_all(cobs_tx).await;
//             let _ = &uart_tx.flush().await;
//         }

//         // Handle RSSI updates
//         if RSSI_VALUE_MPU_TX.load(core::sync::atomic::Ordering::Relaxed) {
//             send_buf.fill(0); // Clear the buffer from any previous data

//             // Reset flag for sending to MPU
//             RSSI_VALUE_MPU_TX.store(false, core::sync::atomic::Ordering::Relaxed);

//             let rssi = RSSI_VALUE.load(core::sync::atomic::Ordering::Relaxed);
//             info!("Sending back RSSI: {}", rssi);

//             let msg = HostProtocolMessage::Bluetooth(Bluetooth::SignalStrength(rssi));
//             let cobs_tx = to_slice_cobs(&msg, &mut send_buf).unwrap();
//             info!("{}", cobs_tx);

//             let _ = uart_tx.write_all(cobs_tx).await;
//             let _ = uart_tx.flush().await;
//             assert_out_irq().await;
//         }

//         // Log performance metrics periodically
//         if timer_pkt.elapsed().as_millis() > 1500 && rx_packet {
//             info!("Total packet number: {}", pkt_counter);
//             info!("Total packet time: {}", timer_tot.elapsed().as_millis() - 1500);
//             info!("Total data incoming: {}", data_counter);
//             if (timer_tot.elapsed().as_secs()) > 0 {
//                 let rate = (data_counter as f32 / (timer_tot.elapsed().as_millis() - 1500) as f32) * 1000.0;
//                 info!("Rough data rate : {}", rate);
//             }
//             data_counter = 0;
//             pkt_counter = 0;
//             rx_packet = false;
//             timer_pkt = Instant::now();
//             timer_tot = Instant::now();
//         }

//         // Handle raw data transfer
//         {
//             if let Ok(data) = BT_DATA_RX.try_receive() {
//                 if !rx_packet {
//                     rx_packet = true;
//                     timer_tot = Instant::now();
//                 }
//                 info!("Infra packet time: {}", timer_pkt.elapsed().as_millis());
//                 timer_pkt = Instant::now();
//                 data_counter += data.len() as u64;
//                 pkt_counter += 1;

//                 let now = Instant::now();
//                 let _ = uart_tx.write_all(data.as_slice()).await;
//                 let _ = uart_tx.flush().await;
//                 info!("Elapsed for packet to UART - {}", now.elapsed().as_micros());

//                 assert_out_irq().await;
//             }
//         }
//         embassy_time::Timer::after_nanos(10).await;
//     }
// }

