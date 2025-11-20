// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

use core::sync::atomic::AtomicBool;

use crate::{server::Server, BT_ADV_CHAN, BT_ADV_CHANGED, BT_DATA_RX, BT_ENABLE, CONNECTION, DEVICE_NAME, IRQ_OUT_PIN, TX_PWR_VALUE};
use consts::{UICR_SECRET_SIZE, UICR_SECRET_START};
use defmt::{debug, error, trace};
use embassy_nrf::{peripherals::SPI0, spis::Spis};
use hmac::{Hmac, Mac};
use host_protocol::{AdvChan, Bluetooth, HostProtocolMessage, PostcardError, SendDataResponse, State, MAX_MSG_SIZE};
use postcard::{from_bytes, to_slice};
use sha2::Sha256 as ShaChallenge;

// This is redundant with BT_ENABLE, only used to report
// the current state over the host-protocol.
static BT_STATE_COPY: AtomicBool = AtomicBool::new(false);

pub struct CommsContext<'a> {
    pub address: [u8; 6],
    pub device_id: [u8; 8],
    pub server: &'a Server,
}

/// Main communication task that handles incoming SPI messages from the MPU
/// Decodes postcard-encoded messages and routes them to appropriate handlers
pub async fn comms_task(mut spi: Spis<'static, SPI0>, context: CommsContext<'_>) -> ! {
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

        let resp = match from_bytes(&req_buf[..n]) {
            Ok(req) => host_protocol_handler(req, &context).await,
            Err(_) => HostProtocolMessage::PostcardError(PostcardError::Deser),
        };
        trace!("Sending response");
        let Ok(resp) = to_slice(&resp, &mut resp_buf[2..]) else {
            error!("Failed to serialize response");
            continue;
        };
        let resp_len = resp.len();
        resp_buf[..2].copy_from_slice(&u16::to_be_bytes(resp_len as u16));
        // Async and blocking perform exactly the same, but an async write
        // makes the subsequent read unreliable.
        let _ = spi.blocking_write_from_ram(&resp_buf[..resp_len + 2]);
    }
}

/// Handles HostProtocol messages received from the MPU
async fn host_protocol_handler<'a>(req: HostProtocolMessage<'a>, context: &CommsContext<'_>) -> HostProtocolMessage<'a> {
    match req {
        HostProtocolMessage::Bluetooth(bluetooth_msg) => {
            trace!("Received HostProtocolMessage::Bluetooth");
            match bluetooth_msg {
                Bluetooth::DisableChannels(chan) => {
                    trace!("DisableChannels");
                    if chan == AdvChan::all() {
                        HostProtocolMessage::Bluetooth(Bluetooth::NackDisableChannels)
                    } else {
                        BT_ADV_CHAN.store(chan.bits(), core::sync::atomic::Ordering::Relaxed);
                        HostProtocolMessage::Bluetooth(Bluetooth::AckDisableChannels)
                    }
                }
                Bluetooth::Enable => {
                    trace!("Enabled");
                    BT_ENABLE.signal(true);
                    BT_STATE_COPY.store(true, core::sync::atomic::Ordering::Relaxed);
                    HostProtocolMessage::Bluetooth(Bluetooth::AckEnable)
                }
                Bluetooth::Disable => {
                    trace!("Disabled");
                    // clean disconnect if connected
                    if let Some(connection) = CONNECTION.read().await.as_ref() {
                        let _ = connection.disconnect();
                    }
                    BT_ENABLE.signal(false);
                    BT_STATE_COPY.store(false, core::sync::atomic::Ordering::Relaxed);
                    HostProtocolMessage::Bluetooth(Bluetooth::AckDisable)
                }
                Bluetooth::GetSignalStrength => {
                    trace!("GetSignalStrength");
                    let conn_lock = CONNECTION.read().await;
                    HostProtocolMessage::Bluetooth(Bluetooth::SignalStrength(conn_lock.as_ref().and_then(|c| c.rssi())))
                }
                Bluetooth::GetFirmwareVersion => {
                    trace!("GetFirmwareVersion");
                    let version = env!("CARGO_PKG_VERSION");
                    HostProtocolMessage::Bluetooth(Bluetooth::AckFirmwareVersion { version })
                }
                Bluetooth::GetReceivedData => HostProtocolMessage::Bluetooth(match BT_DATA_RX.try_receive() {
                    Ok(data) => {
                        trace!("GetReceivedData Some");
                        Bluetooth::ReceivedData(data)
                    }
                    Err(_) => {
                        trace!("GetReceivedData None");
                        IRQ_OUT_PIN.lock().await.as_mut().map(|pin| pin.set_high());
                        Bluetooth::NoReceivedData
                    }
                }),
                Bluetooth::SendData(data) => HostProtocolMessage::Bluetooth({
                    trace!("SendData Some");
                    let conn_lock = CONNECTION.read().await;
                    if let Some(connection) = &conn_lock.as_ref() {
                        match context.server.send_notify(connection, &data) {
                            Ok(_) => Bluetooth::SendDataResponse(SendDataResponse::Sent),
                            Err(_) => Bluetooth::SendDataResponse(SendDataResponse::BufferFull),
                        }
                    } else {
                        trace!("Not connected");
                        Bluetooth::SendDataResponse(SendDataResponse::BufferFull)
                    }
                }),
                Bluetooth::GetBtAddress => HostProtocolMessage::Bluetooth(Bluetooth::AckBtAddress {
                    bt_address: context.address,
                }),
                Bluetooth::SetTxPower { power } => {
                    trace!("SetTxPower");
                    TX_PWR_VALUE.store(i8::from(power), core::sync::atomic::Ordering::Relaxed);
                    HostProtocolMessage::Bluetooth(Bluetooth::AckTxPower)
                }
                Bluetooth::GetDeviceId => HostProtocolMessage::Bluetooth(Bluetooth::AckDeviceId {
                    device_id: context.device_id,
                }),
                Bluetooth::Disconnect => {
                    trace!("Disconnect");
                    // Get current connection and disconnect if connected
                    if let Some(connection) = CONNECTION.read().await.as_ref() {
                        let _ = connection.disconnect();
                    }
                    HostProtocolMessage::Bluetooth(Bluetooth::AckDisconnect)
                }
                _ => {
                    trace!("Other");
                    HostProtocolMessage::InappropriateMessage(get_state())
                }
            }
        }
        HostProtocolMessage::Reset => {
            trace!("Reset");
            cortex_m::peripheral::SCB::sys_reset();
        }
        HostProtocolMessage::ChallengeRequest { nonce } => {
            trace!("ChallengeRequest");
            hmac_challenge_response(nonce)
        }
        HostProtocolMessage::GetState => {
            trace!("GetState");
            HostProtocolMessage::AckState(get_state())
        }
        _ => {
            trace!("Other");
            HostProtocolMessage::InappropriateMessage(get_state())
        }
    }
}

fn get_state() -> State {
    match BT_STATE_COPY.load(core::sync::atomic::Ordering::Relaxed) {
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
