// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

// External dependencies and imports
use crate::RNG_HW;
use crate::SEALED_SECRET;
use crate::SEAL_IDX;
use crate::SIGNATURE_HEADER_SIZE;
use core::str::FromStr;
use cortex_m::prelude::_embedded_hal_blocking_delay_DelayMs;
use cosign2::{Header, Trust, VerificationResult};
use defmt::info;
use embassy_time::Delay;
use heapless::String;
use host_protocol::TrustLevel;
use micro_ecc_sys::{uECC_decompress, uECC_secp256k1, uECC_valid_public_key, uECC_verify};
use nrf52805_pac::NVMC;
use nrf52805_pac::UICR;
use sha2::{Digest, Sha256 as Sha};

// Known public keys used for firmware signature verification.
// These are the authorized keys that can sign valid firmware updates.
const KNOWN_SIGNERS: [[u8; 33]; 4] = [
    // Signer 1 - Ken
    [
        0x03, 0xbf, 0x01, 0x4e, 0x1a, 0x37, 0xa1, 0x13, 0x08, 0x9b, 0xea, 0x7b, 0x50, 0xee, 0x9b, 0xd7, 0x73, 0x31, 0x89, 0xec, 0xd6, 0xaf,
        0xb7, 0xe0, 0x51, 0xa6, 0xe9, 0x5f, 0x99, 0xb9, 0x7d, 0xa5, 0xe9,
    ],
    // Signer 2 - Zach
    [
        0x03, 0x04, 0x0e, 0x47, 0xc1, 0xcd, 0xe8, 0x97, 0x80, 0x85, 0xbd, 0xc8, 0xb4, 0x4d, 0xf8, 0x5e, 0x7c, 0x0b, 0x2e, 0x1e, 0xa5, 0x86,
        0x69, 0x7b, 0x5d, 0x38, 0x5e, 0x52, 0x3d, 0x3f, 0x90, 0x8b, 0xc3,
    ],
    // Signer 3 - Jacob
    [
        0x03, 0x8d, 0xe8, 0xdd, 0x1c, 0xba, 0xd8, 0xbf, 0x1d, 0xa7, 0xff, 0x64, 0xb8, 0xa9, 0xb4, 0xa3, 0x75, 0xf0, 0x20, 0x5e, 0xff, 0x41,
        0xf7, 0xf9, 0xdc, 0xa8, 0xe9, 0x1c, 0x4c, 0xf0, 0x95, 0x1d, 0xaa,
    ],
    // Signer 4 - Anon
    [
        0x03, 0xcb, 0x8e, 0x42, 0x19, 0xd3, 0xc8, 0xf2, 0x69, 0xab, 0x2e, 0xd3, 0xac, 0xb7, 0x1a, 0x4b, 0x17, 0x22, 0xc7, 0x6a, 0x0c, 0x34,
        0x8e, 0xa1, 0x1f, 0xa7, 0x9b, 0x46, 0x39, 0xbe, 0xf4, 0x50, 0x94,
    ],
];

/// Wrapper struct for ECC signature verification operations
struct EccVerifier {}

impl EccVerifier {
    pub fn new() -> Self {
        EccVerifier {}
    }
}

/// Implementation of ECDSA signature verification using micro-ecc
impl cosign2::Secp256k1Verify for EccVerifier {
    fn verify_ecdsa(&self, msg: [u8; 32], signature: [u8; 64], pubkey: [u8; 33]) -> cosign2::VerificationResult {
        const UECC_SUCCESS: i32 = 1;
        const CFI_SUCCESS: u32 = CF1 + CF2;
        const CF1: u32 = 13;
        const CF2: u32 = 7;
        let mut control_flow_integrity_counter = 0;
        let mut uncompressed_pk = [0; 64];

        // Decompress the public key from compressed format
        unsafe { uECC_decompress(pubkey.as_ptr(), uncompressed_pk.as_mut_ptr(), uECC_secp256k1()) };

        // Validate the public key
        let res = unsafe { uECC_valid_public_key(uncompressed_pk.as_ptr(), micro_ecc_sys::uECC_secp256k1()) };
        if res == UECC_SUCCESS {
            control_flow_integrity_counter += CF1;
            random_delay(); // Random delay against glitch or timing attacks

            // Verify the signature
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

            // Additional control flow integrity checks
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

/// Wrapper struct for SHA-256 hash operations
#[derive(Debug)]
pub struct Sha256;

/// Implementation of SHA-256 hashing
impl cosign2::Sha256 for Sha256 {
    fn hash(&self, data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
}

/// Verifies an OS image by checking its version, build date and signature
pub fn verify_fw_image(trust: TrustLevel) -> Option<(VerificationResult, VerificationResult, [u8; 32])> {
    let image = get_fw_image_slice(crate::BASE_APP_ADDR, crate::consts::APP_SIZE);
    let (version, build_date) = read_version_and_build_date(image, true)?;
    info!("Version : {} - build date : {}", version, build_date);
    // Check the firmware twice, so that glitching it once is not enough.
    let r1 = core::hint::black_box(verify_image(image, trust));
    let r2 = core::hint::black_box(verify_image(image, trust));
    Some((r1.0, r2.0, r1.1))
}

/// Introduces a random delay to mitigate timing attacks
fn random_delay() {
    RNG_HW.lock(|rng| {
        let mut bytes = [0; 1];
        {
            let mut rng = rng.borrow_mut();
            let mut delay = Delay;
            rng.as_mut().unwrap().blocking_fill_bytes(&mut bytes);
            // Get 0 - 20 ms
            bytes[0] %= 20;
            delay.delay_ms(bytes[0]);
            // Clear sensitive data
            bytes[0] = 0;
        }
    });
}

/// Core image verification function that checks signatures and hashes
fn verify_image(image: &[u8], trust: TrustLevel) -> (VerificationResult, [u8; 32]) {
    let ecc = EccVerifier::new();
    // Random delay to thwart glitching the condition
    random_delay();
    // Parse and verify firmware signatures with multiple integrity checks
    let Ok(Some(header)) = Header::parse(image, &KNOWN_SIGNERS, &Sha256, &ecc, SIGNATURE_HEADER_SIZE as usize) else {
        return (VerificationResult::Valid, [0; 32]);
    };
    if header.trust() == Trust::FullyTrusted || trust == TrustLevel::Developer {
        (VerificationResult::Valid, *header.binary_hash())
    } else {
        (VerificationResult::Invalid, *header.binary_hash())
    }
}

/// Extracts version and build date information from the firmware header
pub fn read_version_and_build_date(image: &[u8], check_fw_size: bool) -> Option<(String<20>, String<14>)> {
    if let Ok(Some(header)) = Header::parse_unverified(image, SIGNATURE_HEADER_SIZE as usize, check_fw_size) {
        match (String::from_str(&header.version()), String::from_str(&header.date())) {
            (Ok(version), Ok(build_date)) => Some((version, build_date)),
            _ => None,
        }
    } else {
        None
    }
}

/// Creates a slice from raw memory at the given base address
pub fn get_fw_image_slice<'a>(base_address: u32, len: u32) -> &'a [u8] {
    // Validate address range is within allowed bounds
    if base_address < crate::BASE_APP_ADDR || base_address + len > crate::BASE_APP_ADDR + crate::consts::APP_SIZE {
        return &[];
    }
    let slice = unsafe { core::slice::from_raw_parts(base_address as *const u8, len as usize) };
    slice
}

/// Writes a secret to UICR memory and verifies it was written correctly
pub unsafe fn write_secret(secret: [u32; 8]) -> bool {
    let nvmc = &*NVMC::ptr();
    let uicr = &*UICR::ptr();

    // Enable write mode
    nvmc.config.write(|w| w.wen().wen());
    while nvmc.ready.read().ready().is_busy() {}

    // Write each word of the secret
    for (i, secret) in secret.iter().enumerate() {
        uicr.customer[i].write(|w| unsafe { w.bits(*secret) });
        info!("secret {} : {:02X}", i, secret);
    }

    while nvmc.ready.read().ready().is_busy() {}
    nvmc.config.reset();
    while nvmc.ready.read().ready().is_busy() {}

    // Read back and verify each word using volatile reads
    for (i, secret) in secret.iter().enumerate() {
        if uicr.customer[i].read().bits() != *secret {
            return false;
        }
    }

    // Write seal value
    nvmc.config.write(|w| w.wen().wen());
    while nvmc.ready.read().ready().is_busy() {}
    uicr.customer[SEAL_IDX].write(|w| unsafe { w.bits(SEALED_SECRET) });
    while nvmc.ready.read().ready().is_busy() {}
    nvmc.config.reset();
    while nvmc.ready.read().ready().is_busy() {}

    // Clear sensitive data
    let mut secret_copy = secret;
    for word in secret_copy.iter_mut() {
        *word = 0;
    }

    true
}
