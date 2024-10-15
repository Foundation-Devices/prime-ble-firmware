// use crate::RNG_HW;
use crate::ack_msg_send;
use crate::RNG_HW;
use crate::SEALED_SECRET;
use crate::SEAL_IDX;
use cortex_m::prelude::_embedded_hal_blocking_delay_DelayMs;
use cosign2::{Header, VerificationResult};
use defmt::info;
use embassy_nrf::peripherals::UARTE0;
use embassy_nrf::uarte::UarteTx;
use embassy_time::Delay;
use host_protocol::{Bootloader, HostProtocolMessage};
use micro_ecc_sys::{uECC_decompress, uECC_secp256k1, uECC_valid_public_key, uECC_verify};
use nrf52805_pac::NVMC;
use nrf52805_pac::UICR;
use sha2::{Digest, Sha256 as Sha};

// TODO: put well-known public keys here
const KNOWN_SIGNERS: [[u8; 33]; 2] = [
    [
        0x03, 129, 12, 122, 122, 122, 65, 228, 183, 129, 52, 56, 71, 10, 150, 103, 66, 200, 6, 209, 224, 28, 160, 234, 138, 182, 222, 152,
        240, 216, 242, 176, 35,
    ],
    [
        3, 183, 43, 173, 167, 178, 160, 111, 147, 27, 96, 177, 191, 221, 111, 147, 88, 112, 199, 126, 37, 79, 232, 178, 65, 192, 8, 185,
        71, 42, 215, 48, 85,
    ],
];
struct EccVerifier {}

impl EccVerifier {
    pub fn new() -> Self {
        EccVerifier {}
    }
}

impl cosign2::Secp256k1Verify for EccVerifier {
    fn verify_ecdsa(&self, msg: [u8; 32], signature: [u8; 64], pubkey: [u8; 33]) -> cosign2::VerificationResult {
        const UECC_SUCCESS: i32 = 1;
        const CFI_SUCCESS: u32 = CF1 + CF2;
        const CF1: u32 = 13;
        const CF2: u32 = 7;
        let mut control_flow_integrity_counter = 0;
        let mut uncompressed_pk = [0; 64];
        unsafe { uECC_decompress(pubkey.as_ptr(), uncompressed_pk.as_mut_ptr(), uECC_secp256k1()) };
        let res = unsafe { uECC_valid_public_key(uncompressed_pk.as_ptr(), micro_ecc_sys::uECC_secp256k1()) };
        if res == UECC_SUCCESS {
            control_flow_integrity_counter += CF1;
            random_delay(); // Random delay against glitch or timing attacks
            let res = unsafe {
                uECC_verify(
                    uncompressed_pk.as_ptr(),
                    msg.as_ptr(),
                    msg.len() as u32,
                    signature.as_ptr(),
                    uECC_secp256k1(),
                )
            };
            random_delay(); // Random delay against glitch or timing attacks
            if res == UECC_SUCCESS {
                control_flow_integrity_counter += CF2;
                let complement = !UECC_SUCCESS;
                let complement_ptr = &complement as *const i32;
                if !res == unsafe { complement_ptr.read_volatile() } && control_flow_integrity_counter == CFI_SUCCESS {
                    return cosign2::VerificationResult::Valid;
                }
            }
        }
        cosign2::VerificationResult::Invalid
    }
}

#[derive(Debug)]
pub struct Sha256 {
    pub sha: [u8; 32],
}

impl cosign2::Sha256 for Sha256 {
    fn hash(&self, data: &[u8]) -> [u8; 32] {
        Sha::digest(data).into()
    }
}

pub(crate) fn verify_os_image(image: &[u8]) -> Option<(VerificationResult, Sha256)> {
    if let Some((mut version, mut build_date)) = read_version_and_build_date(image) {
        info!(
            "Version : {} - build date : {}",
            version.make_ascii_lowercase(),
            build_date.make_ascii_lowercase()
        );
        let (verif_res, hash) = verify_image(image);
        return Some((verif_res, hash));
    }
    None
}

fn random_delay() {
    RNG_HW.lock(|rng| {
        let mut bytes = [0; 1];
        {
            let mut rng = rng.borrow_mut();
            let mut delay = Delay;
            rng.as_mut().unwrap().blocking_fill_bytes(&mut bytes);
            // Get 0 - 200 ms
            bytes[0] %= 200;
            delay.delay_ms(bytes[0]);
        }
    });
}

fn verify_image(image: &[u8]) -> (VerificationResult, Sha256) {
    let mut control_flow_integrity_counter = 0;
    const CF1: u32 = 3;
    const CF2: u32 = 5;
    const CF3: u32 = 7;
    const CF4: u32 = 11;
    const CF5: u32 = 13;
    const CF6: u32 = 17;
    let ecc = EccVerifier::new();
    let sha = Sha256 { sha: [0; 32] };
    // Random delay to thwart glitching the condition
    random_delay();
    // Parse and verify firmware signatures
    let res = Header::parse(image, &KNOWN_SIGNERS, &sha, &ecc);
    if res.is_ok() {
        control_flow_integrity_counter += CF1;
        if let Ok(Some(header)) = res {
            control_flow_integrity_counter += CF2;
            if *header.firmware_hash() != [0; 32] {
                control_flow_integrity_counter += CF3;
                if image.len() > Header::SIZE {
                    control_flow_integrity_counter += CF4;
                    let firmware_bytes = &image[Header::SIZE..];
                    #[allow(clippy::collapsible_if)]
                    if firmware_bytes.len() as u32 == header.firmware_size() {
                        control_flow_integrity_counter += CF5;
                        if core::hint::black_box(firmware_bytes.len() as u32 == header.firmware_size()) {
                            control_flow_integrity_counter += CF6;
                            let cfi_counter_ptr = &control_flow_integrity_counter as *const u32;
                            if unsafe { cfi_counter_ptr.read_volatile() } == CF1 + CF2 + CF3 + CF4 + CF5 + CF6 {
                                let sha256 = header.firmware_hash();
                                return (VerificationResult::Valid, Sha256 { sha: *sha256 });
                            }
                        }
                    }
                }
            }
        }
    }
    (VerificationResult::Invalid, Sha256 { sha: [0; 32] })
}

fn read_version_and_build_date(image: &[u8]) -> Option<([u8; 20], [u8; 14])> {
    if let Ok(Some(header)) = Header::parse_unverified(image) {
        let mut version_bytes = [0u8; 20];
        let str_bytes = header.version().as_bytes();
        version_bytes[..str_bytes.len()].copy_from_slice(str_bytes);

        let mut date_bytes = [0u8; 14];
        let str_bytes = header.date().as_bytes();
        date_bytes[..str_bytes.len()].copy_from_slice(str_bytes);

        return Some((version_bytes, date_bytes));
    }
    None
}

pub fn get_fw_image_slice<'a>(base_address: u32, len: u32) -> &'a [u8] {
    let slice = unsafe { core::slice::from_raw_parts(base_address as *const u8, len as usize) };
    slice
}

pub fn check_fw(image_slice: &[u8], tx: &mut UarteTx<UARTE0>) -> Option<bool> {
    if let Some((result, hash)) = verify_os_image(image_slice) {
        if result == VerificationResult::Valid {
            ack_msg_send(
                HostProtocolMessage::Bootloader(Bootloader::AckVerifyFirmware {
                    result: true,
                    hash: hash.sha,
                }),
                tx,
            );
            let _ = tx.blocking_write(b"Fw image valid");

            return Some(true);
        } else {
            info!("Invalid signature!");
            let _ = tx.blocking_write(b"Fw image not valid");

            ack_msg_send(
                HostProtocolMessage::Bootloader(Bootloader::AckVerifyFirmware {
                    result: false,
                    hash: hash.sha,
                }),
                tx,
            );
            return Some(false);
        }
    }
    None
}

pub unsafe fn write_secret(secret: [u32; 4]) -> bool {
    let nvmc = &*NVMC::ptr();
    let uicr = &*UICR::ptr();
    nvmc.config.write(|w| w.wen().wen());
    while nvmc.ready.read().ready().is_busy() {}
    uicr.customer[0].write(|w| unsafe { w.bits(secret[0]) });
    info!("secret 0 : {:02X}", secret[0]);
    uicr.customer[1].write(|w| unsafe { w.bits(secret[1]) });
    info!("secret 1 : {:02X}", secret[1]);
    uicr.customer[2].write(|w| unsafe { w.bits(secret[2]) });
    info!("secret 2 : {:02X}", secret[2]);
    uicr.customer[3].write(|w| unsafe { w.bits(secret[3]) });
    info!("secret 3 : {:02X}", secret[3]);
    while nvmc.ready.read().ready().is_busy() {}
    nvmc.config.reset();
    while nvmc.ready.read().ready().is_busy() {}

    // Read back and check secret
    let tmp = uicr.customer[0].read().bits();
    if tmp != secret[0] {
        return false;
    }
    let tmp = uicr.customer[1].read().bits();
    if tmp != secret[1] {
        return false;
    }
    let tmp = uicr.customer[2].read().bits();
    if tmp != secret[2] {
        return false;
    }
    let tmp = uicr.customer[3].read().bits();
    if tmp != secret[3] {
        return false;
    }
    nvmc.config.write(|w| w.wen().wen());
    while nvmc.ready.read().ready().is_busy() {}
    uicr.customer[SEAL_IDX].write(|w| unsafe { w.bits(SEALED_SECRET) });
    while nvmc.ready.read().ready().is_busy() {}
    nvmc.config.reset();
    while nvmc.ready.read().ready().is_busy() {}

    true
}
