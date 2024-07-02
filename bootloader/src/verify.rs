use micro_ecc_sys::{uECC_decompress, uECC_secp256k1, uECC_valid_public_key, uECC_verify};
use sha2::Sha256;


// TODO: put well-known public keys here
const KNOWN_SIGNERS: [[u8; 33]; 0] = [
    // [0; 33],
];

struct EccVerifier {}

impl EccVerifier {
    pub fn new() -> Self {
        EccVerifier {}
    }
}

impl cosign2::Secp256k1Verify for EccVerifier {
    fn verify_ecdsa(
        &self,
        msg: [u8; 32],
        signature: [u8; 64],
        pubkey: [u8; 33],
    ) -> cosign2::VerificationResult {
        const UECC_SUCCESS: i32 = 1;
        const CFI_SUCCESS: u32 = CF1 + CF2;
        const CF1: u32 = 13;
        const CF2: u32 = 7;
        let mut control_flow_integrity_counter = 0;

        let mut uncompressed_pk = [0; 64];

        unsafe {
            uECC_decompress(
                pubkey.as_ptr(),
                uncompressed_pk.as_mut_ptr(),
                uECC_secp256k1(),
            )
        };

        let res = unsafe {
            uECC_valid_public_key(uncompressed_pk.as_ptr(), micro_ecc_sys::uECC_secp256k1())
        };
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
                if !res == unsafe { complement_ptr.read_volatile() }
                    && control_flow_integrity_counter == CFI_SUCCESS
                {
                    return cosign2::VerificationResult::Valid;
                }
            }
        }

        cosign2::VerificationResult::Invalid
    }
}

struct Sha256 {
    sha: Sha,
}


impl cosign2::Sha256 for Sha256 {
    fn hash(&self, data: &[u8]) -> [u8; 32] {
        self.sha.reset();
        self.sha
            .sha256_cb(data, SHA_PROGRESS_BLOCK_FREQ, |curr, total| {
                progress_bar_set(ProgressBarPart::Verifying, curr as u32, total as u32)
            })
            .0
    }
}

pub(crate) fn verify_os_image(kind: BootImageKind, image: &[u8]) -> VerificationResult {
    if let Some((version, build_date)) = read_version_and_build_date(image) {
        unsafe {
            match kind {
                BootImageKind::Main => {
                    crate::systeminfo::MAIN_OS_VERSION.replace(version);
                    crate::systeminfo::MAIN_OS_BUILD_DATE.replace(build_date);
                }
                BootImageKind::Recovery => {
                    crate::systeminfo::RECOVERY_OS_VERSION.replace(version);
                    crate::systeminfo::RECOVERY_OS_BUILD_DATE.replace(build_date);
                }
            };
        }

        let (verif_res, hash) = verify_image(image);

        match kind {
            BootImageKind::Main => unsafe {
                crate::systeminfo::MAIN_OS_IMAGE_HASH.replace(hash);
                crate::systeminfo::MAIN_OS_VERIFICATION_RESULT = verif_res;
            },
            BootImageKind::Recovery => unsafe {
                crate::systeminfo::RECOVERY_OS_IMAGE_HASH.replace(hash);
                crate::systeminfo::RECOVERY_OS_VERIFICATION_RESULT = verif_res;
            },
        }

        return verif_res;
    }

    VerificationResult::Invalid
}

fn verify_image(image: &[u8]) -> (VerificationResult, Sha256Hash) {
    let mut control_flow_integrity_counter = 0;
    const CF1: u32 = 3;
    const CF2: u32 = 5;
    const CF3: u32 = 7;
    const CF4: u32 = 11;
    const CF5: u32 = 13;
    const CF6: u32 = 17;

    let ecc = EccVerifier::new();
    let sha = Sha256::new();

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

                        if core::hint::black_box(
                            firmware_bytes.len() as u32 == header.firmware_size(),
                        ) {
                            control_flow_integrity_counter += CF6;

                            let cfi_counter_ptr = &control_flow_integrity_counter as *const u32;
                            if unsafe { cfi_counter_ptr.read_volatile() }
                                == CF1 + CF2 + CF3 + CF4 + CF5 + CF6
                            {
                                let mut hash = [0u8; 32];
                                hash.copy_from_slice(header.firmware_hash());
                                return (VerificationResult::Valid, Sha256Hash(hash));
                            }
                        }
                    }
                }
            }
        }
    }

    (VerificationResult::Invalid, Sha256Hash([0; 32]))
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