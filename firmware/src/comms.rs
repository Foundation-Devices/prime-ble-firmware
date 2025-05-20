// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{BT_ADDRESS, BT_DATA_TX, IRQ_OUT_PIN};
use crate::{BT_ADV_CHAN, BT_DATA_RX, BT_STATE, RSSI_VALUE};
use consts::{APP_MTU, BT_MAX_NUM_PKT, UICR_SECRET_SIZE, UICR_SECRET_START};
use defmt::{debug, error, trace};
#[cfg(not(feature = "hw-rev-d"))]
use embassy_nrf::{
    buffered_uarte::{BufferedUarte, BufferedUarteTx},
    peripherals::{TIMER1, UARTE0},
};
#[cfg(feature = "hw-rev-d")]
use embassy_nrf::{peripherals::SPI0, spis::Spis};
#[cfg(feature = "analytics")]
use embassy_time::Instant;
#[cfg(feature = "hw-rev-d")]
use embassy_time::Timer;
#[cfg(not(feature = "hw-rev-d"))]
use embassy_time::{with_timeout, Duration};
#[cfg(not(feature = "hw-rev-d"))]
use embedded_io_async::Write;
use heapless::Vec;
use hmac::{Hmac, Mac};
use host_protocol::{AdvChan, Bluetooth, HostProtocolMessage, PostcardError, SendDataResponse, State, COBS_MAX_MSG_SIZE};
#[cfg(not(feature = "hw-rev-d"))]
use postcard::{
    accumulator::{CobsAccumulator, FeedResult},
    to_slice_cobs,
};
#[cfg(feature = "hw-rev-d")]
use postcard::{from_bytes, to_slice};
use sha2::Sha256 as ShaChallenge;

/// Helper function to signal the MPU via GPIO
/// Sends a falling edge pulse on the IRQ line
async fn assert_out_irq() {
    let irq_out = IRQ_OUT_PIN.lock().await;
    {
        let mut pin = irq_out.borrow_mut();
        // Generate falling edge pulse
        pin.as_mut().unwrap().set_low();
        pin.as_mut().unwrap().set_high();
    }
}

#[cfg(feature = "analytics")]
/// Logs performance metrics if 1.5s has passed since the last log
fn log_performance(timer_pkt: &mut Instant, rx_packet: &mut bool, pkt_counter: &mut u64, data_counter: &mut u64, timer_tot: &mut Instant) {
    if timer_pkt.elapsed().as_millis() > 1500 && *rx_packet {
        let pkt_time = timer_tot.elapsed().as_millis() - 1500;
        debug!(
            "Total packet number: {}, time: {} ms, data incoming: {} bytes",
            pkt_counter, pkt_time, data_counter
        );
        if (timer_tot.elapsed().as_secs()) > 0 {
            let rate = (*data_counter as f32 / pkt_time as f32) * 8.0;
            debug!("Rough data rate: {} kbps", rate);
        }
        *data_counter = 0;
        *pkt_counter = 0;
        *rx_packet = false;
        *timer_pkt = Instant::now();
        *timer_tot = Instant::now();
    }
}

#[cfg(feature = "analytics")]
/// Logs the time taken to process an infra packet
fn log_infra_packet(
    timer_pkt: &mut Instant,
    rx_packet: &mut bool,
    data_counter: &mut u64,
    pkt_counter: &mut u64,
    timer_tot: &mut Instant,
    data: &[u8],
) {
    if !*rx_packet {
        *rx_packet = true;
        *timer_tot = Instant::now();
    }
    debug!("Infra packet time: {}", timer_pkt.elapsed().as_millis());
    *timer_pkt = Instant::now();
    *data_counter += data.len() as u64;
    *pkt_counter += 1;
}

/// Main communication task that handles incoming UART messages from the MPU
/// Decodes COBS-encoded messages and routes them to appropriate handlers
#[cfg(not(feature = "hw-rev-d"))]
#[embassy_executor::task]
pub async fn comms_task(uart: BufferedUarte<'static, UARTE0, TIMER1>) {
    // Rough performance metrics
    #[cfg(feature = "analytics")]
    let mut data_counter: u64 = 0;
    #[cfg(feature = "analytics")]
    let mut pkt_counter: u64 = 0;
    #[cfg(feature = "analytics")]
    let mut rx_packet = false;
    #[cfg(feature = "analytics")]
    let mut timer_pkt: Instant = Instant::now();
    #[cfg(feature = "analytics")]
    let mut timer_tot: Instant = Instant::now();

    let mut resp_buf = [0u8; COBS_MAX_MSG_SIZE];

    // Split UART into RX and TX
    let (mut rx, mut tx) = uart.split();

    // Buffer for raw incoming UART data
    let mut raw_buf = [0u8; 64];

    // COBS accumulator for decoding incoming messages
    let mut cobs_buf: CobsAccumulator<COBS_MAX_MSG_SIZE> = CobsAccumulator::new();
    loop {
        #[cfg(feature = "analytics")]
        log_performance(&mut timer_pkt, &mut rx_packet, &mut pkt_counter, &mut data_counter, &mut timer_tot);

        // Read data from UART
        if let Ok(n) = with_timeout(Duration::from_micros(200), rx.read(&mut raw_buf)).await {
            // Clear the response buffer
            resp_buf.fill(0);

            // Exit if no data received
            let Ok(num) = n else {
                continue;
            };

            let buf = &raw_buf[..num];
            let mut window = buf;
            let mut resp: Option<HostProtocolMessage>;

            // Process all complete COBS messages in the buffer
            'cobs: while !window.is_empty() {
                (window, resp) = match cobs_buf.feed_ref::<HostProtocolMessage>(window) {
                    FeedResult::Consumed => {
                        break 'cobs;
                    }
                    FeedResult::OverFull(new_wind) => {
                        trace!("overfull");
                        (new_wind, Some(HostProtocolMessage::PostcardError(PostcardError::OverFull)))
                    }
                    FeedResult::DeserError(new_wind) => {
                        trace!("DeserError");
                        (new_wind, Some(HostProtocolMessage::PostcardError(PostcardError::Deser)))
                    }
                    FeedResult::Success { data, remaining } => {
                        trace!("Success");
                        debug!("Remaining {} bytes", remaining.len());
                        // Route message to appropriate handler based on type
                        (remaining, host_protocol_handler(data, &mut resp_buf).await)
                    }
                };

                if let Some(msg) = resp {
                    let mut buf = [0u8; COBS_MAX_MSG_SIZE];

                    if let Ok(resp) = to_slice_cobs(&msg, &mut buf) {
                        let _ = tx.write_all(resp).await;
                        let _ = tx.flush().await;
                        assert_out_irq().await;
                    }
                }
            }
        }
    }
}

/// Main communication task that handles incoming SPI messages from the MPU
/// Decodes postcard-encoded messages and routes them to appropriate handlers
#[cfg(feature = "hw-rev-d")]
#[embassy_executor::task]
pub async fn comms_task(mut spi: Spis<'static, SPI0>) {
    // Rough performance metrics
    #[cfg(feature = "analytics")]
    let mut data_counter: u64 = 0;
    #[cfg(feature = "analytics")]
    let mut pkt_counter: u64 = 0;
    #[cfg(feature = "analytics")]
    let mut rx_packet = false;
    #[cfg(feature = "analytics")]
    let mut timer_pkt: Instant = Instant::now();
    #[cfg(feature = "analytics")]
    let mut timer_tot: Instant = Instant::now();

    let mut resp_buf = [0u8; COBS_MAX_MSG_SIZE];

    // Buffer for raw incoming SPI data
    let mut raw_buf = [0u8; 64];

    loop {
        #[cfg(feature = "analytics")]
        log_performance(&mut timer_pkt, &mut rx_packet, &mut pkt_counter, &mut data_counter, &mut timer_tot);

        // Clear the response buffer
        resp_buf.fill(0);

        // Read data from SPI
        let res = spi.read(&mut raw_buf).await;

        // Exit if no data received
        let Ok(n) = res else {
            error!("Failed to read from SPI");
            continue;
        };

        let buf = &raw_buf[..n];
        if let Some(resp) = match from_bytes(buf) {
            Ok(req) => host_protocol_handler(req, &mut resp_buf).await,
            Err(_) => Some(HostProtocolMessage::PostcardError(PostcardError::Deser)),
        } {
            trace!("Sending response");
            let mut buf = [0u8; COBS_MAX_MSG_SIZE];
            if let Ok(resp) = to_slice(&resp, &mut buf) {
                assert_out_irq().await;
                let resp_len = u16::to_be_bytes(resp.len() as u16);
                let _ = spi.write(&resp_len).await;
                let _ = spi.write(resp).await;
            } else {
                error!("Failed to serialize response");
            }
        }
    }
}

#[cfg(feature = "hw-rev-d")]
#[embassy_executor::task]
pub async fn check_ble_rx_task() {
    loop {
        if !BT_DATA_RX.is_empty() {
            assert_out_irq().await;
        }
        Timer::after_millis(50).await;
    }
}

/// Handles HostProtocol messages received from the MPU
async fn host_protocol_handler<'a>(req: HostProtocolMessage<'a>, resp_buf: &'a mut [u8]) -> Option<HostProtocolMessage<'a>> {
    match req {
        HostProtocolMessage::Bluetooth(bluetooth_msg) => {
            trace!("Received HostProtocolMessage::Bluetooth");
            match bluetooth_msg {
                Bluetooth::DisableChannels(chan) => {
                    trace!("DisableChannels");
                    if chan == AdvChan::all() {
                        Some(HostProtocolMessage::Bluetooth(Bluetooth::NackDisableChannels))
                    } else {
                        BT_ADV_CHAN.store(chan.bits(), core::sync::atomic::Ordering::Relaxed);
                        Some(HostProtocolMessage::Bluetooth(Bluetooth::AckDisableChannels))
                    }
                }
                Bluetooth::Enable => {
                    trace!("Enabled");
                    BT_STATE.store(true, core::sync::atomic::Ordering::Relaxed);
                    Some(HostProtocolMessage::Bluetooth(Bluetooth::AckEnable))
                }
                Bluetooth::Disable => {
                    trace!("Disabled");
                    BT_STATE.store(false, core::sync::atomic::Ordering::Relaxed);
                    Some(HostProtocolMessage::Bluetooth(Bluetooth::AckDisable))
                }
                Bluetooth::GetSignalStrength => {
                    trace!("GetSignalStrength");
                    let rssi = RSSI_VALUE.load(core::sync::atomic::Ordering::Relaxed);
                    Some(HostProtocolMessage::Bluetooth(Bluetooth::SignalStrength(if rssi == i8::MIN {
                        None
                    } else {
                        Some(rssi)
                    })))
                }
                Bluetooth::GetFirmwareVersion => {
                    trace!("GetFirmwareVersion");
                    let version = env!("CARGO_PKG_VERSION");
                    Some(HostProtocolMessage::Bluetooth(Bluetooth::AckFirmwareVersion { version }))
                }
                Bluetooth::GetReceivedData => Some(HostProtocolMessage::Bluetooth(if let Ok(data) = BT_DATA_RX.try_receive() {
                    trace!("GetReceivedData Some");
                    let len = data.len();
                    resp_buf[..len].copy_from_slice(data.as_slice());
                    Bluetooth::ReceivedData(&resp_buf[..len])
                } else {
                    trace!("GetReceivedData None");
                    Bluetooth::NoReceivedData
                })),
                Bluetooth::SendData(data) => Some(HostProtocolMessage::Bluetooth(if data.len() <= APP_MTU {
                    trace!("SendData Some");
                    // Only accept data packets within APP_MTU size limit
                    let mut buffer_tx_bt = BT_DATA_TX.lock().await;
                    if buffer_tx_bt.len() < BT_MAX_NUM_PKT {
                        if buffer_tx_bt.push(Vec::from_slice(data).unwrap()).is_err() {
                            Bluetooth::SendDataResponse(SendDataResponse::BufferFull)
                        } else {
                            Bluetooth::SendDataResponse(SendDataResponse::Sent)
                        }
                    } else {
                        trace!("SendData Full");
                        Bluetooth::SendDataResponse(SendDataResponse::BufferFull)
                    }
                } else {
                    trace!("SendData TooLarge");
                    Bluetooth::SendDataResponse(SendDataResponse::DataTooLarge)
                })),
                Bluetooth::GetBtAddress => Some(HostProtocolMessage::Bluetooth(Bluetooth::AckBtAddress {
                    bt_address: *BT_ADDRESS.lock().await,
                })),
                _ => {
                    trace!("Other");
                    Some(HostProtocolMessage::InappropriateMessage(get_state()))
                }
            }
        }
        HostProtocolMessage::Reset => {
            trace!("Reset");
            cortex_m::peripheral::SCB::sys_reset();
        }
        HostProtocolMessage::ChallengeRequest { nonce } => {
            trace!("ChallengeRequest");
            Some(hmac_challenge_response(nonce))
        }
        HostProtocolMessage::GetState => {
            trace!("GetState");
            Some(HostProtocolMessage::AckState(get_state()))
        }
        _ => {
            trace!("Other");
            None
        }
    }
}

fn get_state() -> State {
    match BT_STATE.load(core::sync::atomic::Ordering::Relaxed) {
        true => State::Enabled,
        false => State::Disabled,
    }
}

/// Handles HMAC challenge-response authentication
fn hmac_challenge_response(nonce: u64) -> HostProtocolMessage<'static> {
    type HmacSha256 = Hmac<ShaChallenge>;
    // Get device secret from UICR memory
    let secret_as_slice = unsafe { core::slice::from_raw_parts(UICR_SECRET_START as *const u8, UICR_SECRET_SIZE as usize) };

    // Calculate HMAC response
    if let Ok(mut mac) = HmacSha256::new_from_slice(secret_as_slice) {
        mac.update(&nonce.to_be_bytes());
        let result = mac.finalize().into_bytes();
        debug!("{=[u8;32]:#X}", result.into());
        HostProtocolMessage::ChallengeResult { result: result.into() }
    } else {
        HostProtocolMessage::ChallengeResult { result: [0xFF; 32] }
    }
}
