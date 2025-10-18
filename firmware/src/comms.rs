// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{BT_ADDRESS, BT_ADV_CHAN, BT_DATA_RX, BT_DATA_TX, BT_MAX_NUM_PKT, BT_STATE, DEVICE_ID, IRQ_OUT_PIN, RSSI_VALUE, TX_PWR_VALUE};
use consts::{UICR_SECRET_SIZE, UICR_SECRET_START};
use defmt::{debug, error, trace};
use embassy_nrf::{peripherals::SPI0, spis::Spis};
#[cfg(feature = "analytics")]
use embassy_time::Instant;
use hmac::{Hmac, Mac};
use host_protocol::{AdvChan, Bluetooth, HostProtocolMessage, PostcardError, SendDataResponse, State, MAX_MSG_SIZE};
use postcard::{from_bytes, to_slice};
use sha2::Sha256 as ShaChallenge;

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

/// Main communication task that handles incoming SPI messages from the MPU
/// Decodes postcard-encoded messages and routes them to appropriate handlers
#[embassy_executor::task]
pub async fn comms_task(mut spi: Spis<'static, SPI0>) {
    // Buffer for raw incoming SPI data
    let mut req_buf = [0u8; MAX_MSG_SIZE];
    let mut resp_buf = [0u8; MAX_MSG_SIZE];

    loop {
        // Read data from SPI
        let res = spi.read(&mut req_buf).await;

        // Exit if no data received
        let Ok(n) = res else {
            error!("Failed to read from SPI");
            continue;
        };

        if let Some(resp) = match from_bytes(&req_buf[..n]) {
            Ok(req) => host_protocol_handler(req).await,
            Err(_) => Some(HostProtocolMessage::PostcardError(PostcardError::Deser)),
        } {
            trace!("Sending response");
            let Ok(resp) = to_slice(&resp, &mut resp_buf[2..]) else {
                error!("Failed to serialize response");
                continue;
            };
            let resp_len = resp.len();
            resp_buf[..2].copy_from_slice(&u16::to_be_bytes(resp_len as u16));
            let _ = spi.blocking_write_from_ram(&resp_buf[..resp_len + 2]);
        }
    }
}

/// Handles HostProtocol messages received from the MPU
async fn host_protocol_handler<'a>(req: HostProtocolMessage<'a>) -> Option<HostProtocolMessage<'a>> {
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
                Bluetooth::GetReceivedData => Some(HostProtocolMessage::Bluetooth(match BT_DATA_RX.try_receive() {
                    Ok(data) => {
                        trace!("GetReceivedData Some");
                        Bluetooth::ReceivedData(data)
                    }
                    Err(_) => {
                        trace!("GetReceivedData None");
                        IRQ_OUT_PIN.lock().await.borrow_mut().as_mut().map(|pin| pin.set_high());
                        Bluetooth::NoReceivedData
                    }
                })),
                Bluetooth::SendData(data) => Some(HostProtocolMessage::Bluetooth({
                    trace!("SendData Some");
                    // Only accept data packets within APP_MTU size limit
                    let mut buffer_tx_bt = BT_DATA_TX.lock().await;
                    if buffer_tx_bt.len() < BT_MAX_NUM_PKT {
                        if buffer_tx_bt.push(data).is_err() {
                            Bluetooth::SendDataResponse(SendDataResponse::BufferFull)
                        } else {
                            Bluetooth::SendDataResponse(SendDataResponse::Sent)
                        }
                    } else {
                        trace!("SendData Full");
                        Bluetooth::SendDataResponse(SendDataResponse::BufferFull)
                    }
                })),
                Bluetooth::GetBtAddress => Some(HostProtocolMessage::Bluetooth(Bluetooth::AckBtAddress {
                    bt_address: *BT_ADDRESS.lock().await,
                })),
                Bluetooth::SetTxPower { power } => {
                    trace!("SetTxPower");
                    TX_PWR_VALUE.store(i8::from(power), core::sync::atomic::Ordering::Relaxed);
                    Some(HostProtocolMessage::Bluetooth(Bluetooth::AckTxPower))
                }
                Bluetooth::GetDeviceId => Some(HostProtocolMessage::Bluetooth(Bluetooth::AckDeviceId {
                    device_id: *DEVICE_ID.lock().await,
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
