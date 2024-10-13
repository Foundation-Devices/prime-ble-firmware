use super::*;

const MONTHS: [&[u8]; 12] = [
    b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov", b"Dec",
];
const DIGITS: [u8; 10] = [b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9'];

const PUBKEY1: [u8; 33] = [0xCD; 33];
const PUBKEY2: [u8; 33] = [0xEF; 33];

struct Secp256k1Sign([u8; 33]);

impl crate::Secp256k1Sign for Secp256k1Sign {
    fn sign_ecdsa(&self, _msg: [u8; 32]) -> [u8; 64] {
        [0xAB; 64]
    }

    fn pubkey(&self) -> [u8; 33] {
        self.0
    }
}

struct Secp256k1Verify;

impl crate::Secp256k1Verify for Secp256k1Verify {
    fn verify_ecdsa(
        &self,
        _msg: [u8; 32],
        _sig: [u8; 64],
        _pubkey: [u8; 33],
    ) -> VerificationResult {
        VerificationResult::Valid
    }
}

struct Sha256;

impl crate::Sha256 for Sha256 {
    fn hash(&self, _data: &[u8]) -> [u8; 32] {
        [1; 32]
    }
}

#[test]
fn developer_singing() {
    let firmware = [0x01, 0x02, 0x03, 0x04];
    let header = Header::sign_new(
        Magic::Atsama5d27KeyOs,
        "1.2.3",
        firmware.len().try_into().unwrap(),
        Signer::Developer,
        &firmware[..],
        &Sha256,
        &Secp256k1Sign(PUBKEY2),
    )
    .unwrap();
    let mut buf = [0u8; Header::SIZE + 4];
    header.serialize(&mut buf).unwrap();
    buf[Header::SIZE..].copy_from_slice(&firmware);
    let parsed = Header::parse(&buf[..], &[PUBKEY2], &Sha256, &Secp256k1Verify)
        .unwrap()
        .unwrap();

    // The parsed header is the same as the serialized header.
    assert_eq!(header.magic(), parsed.magic());
    assert_eq!(header.timestamp(), parsed.timestamp());
    assert_eq!(header.date(), parsed.date());
    assert_eq!(header.version(), parsed.version());
    assert_eq!(header.firmware_size(), parsed.firmware_size());
    assert_eq!(header.pubkey1(), parsed.pubkey1());
    assert_eq!(header.signature1(), parsed.signature1());
    assert_eq!(header.pubkey2(), parsed.pubkey2());
    assert_eq!(header.signature2(), parsed.signature2());

    // The signing procedure filled in the correct field.
    assert_eq!(header.pubkey1(), [0; 33]);
    assert_eq!(header.signature1(), [0; 64]);
    assert_ne!(header.pubkey2(), [0; 33]);
    assert_ne!(header.signature2(), [0; 64]);

    // The binary representation is correct.
    assert_eq!(buf.len(), Header::SIZE + firmware.len());
    // Magic number.
    assert_eq!(buf[..4], [0x50, 0x52, 0x4D, 0x31]);
    // Timestamp.
    assert_eq!(buf[4..8], header.timestamp().to_le_bytes(),);
    // Date.
    assert!(MONTHS.contains(&&buf[8..11]));
    assert_eq!(buf[11], b' ');
    assert!(DIGITS.contains(&buf[12]));
    assert!(DIGITS.contains(&buf[13]));
    assert_eq!(buf[14], b' ');
    assert!(DIGITS.contains(&buf[15]));
    assert!(DIGITS.contains(&buf[16]));
    assert!(DIGITS.contains(&buf[17]));
    assert!(DIGITS.contains(&buf[18]));
    assert_eq!(&buf[19..22], &[0, 0, 0]);
    // Version.
    assert_eq!(&buf[22..42], b"1.2.3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0");
    // Firmware size.
    assert_eq!(buf[42..46], [4, 0, 0, 0]);
    // Public key 1.
    assert_eq!(&buf[46..79], [0; 33]);
    // Signature 1.
    assert_eq!(&buf[79..143], [0; 64]);
    // Public key 2.
    assert_eq!(&buf[143..176], PUBKEY2);
    // Signature 2.
    assert_ne!(buf[176..240], [0; 64]);
    // Reserved.
    assert!(&buf[240..Header::SIZE].iter().all(|&b| b == 0));
}

#[test]
fn trusted_singing() {
    let firmware = [0x01, 0x02, 0x03, 0x04];
    let mut header = Header::sign_new(
        Magic::Atsama5d27KeyOs,
        "1.2.3-alpha1",
        firmware.len().try_into().unwrap(),
        Signer::Trusted,
        &firmware[..],
        &Sha256,
        &Secp256k1Sign(PUBKEY1),
    )
    .unwrap();
    let mut buf = [0u8; Header::SIZE + 4];
    header.serialize(&mut buf).unwrap();
    buf[Header::SIZE..].copy_from_slice(&firmware);
    let parsed = Header::parse(&buf[..], &[PUBKEY1], &Sha256, &Secp256k1Verify)
        .unwrap()
        .unwrap();

    // The parsed header is the same as the serialized header.
    assert_eq!(header.magic(), parsed.magic());
    assert_eq!(header.timestamp(), parsed.timestamp());
    assert_eq!(header.date(), parsed.date());
    assert_eq!(header.version(), parsed.version());
    assert_eq!(header.firmware_size(), parsed.firmware_size());
    assert_eq!(header.pubkey1(), parsed.pubkey1());
    assert_eq!(header.signature1(), parsed.signature1());
    assert_eq!(header.pubkey2(), parsed.pubkey2());
    assert_eq!(header.signature2(), parsed.signature2());

    // The signing procedure filled in the correct field.
    assert_ne!(header.pubkey1(), [0; 33]);
    assert_ne!(header.signature1(), [0; 64]);
    assert_eq!(header.pubkey2(), [0; 33]);
    assert_eq!(header.signature2(), [0; 64]);

    // The binary representation is correct.
    assert_eq!(buf.len(), Header::SIZE + firmware.len());
    // Magic number.
    assert_eq!(buf[..4], [0x50, 0x52, 0x4D, 0x31]);
    // Timestamp.
    assert_eq!(buf[4..8], header.timestamp().to_le_bytes());
    // Date.
    assert!(MONTHS.contains(&&buf[8..11]));
    assert_eq!(buf[11], b' ');
    assert!(DIGITS.contains(&buf[12]));
    assert!(DIGITS.contains(&buf[13]));
    assert_eq!(buf[14], b' ');
    assert!(DIGITS.contains(&buf[15]));
    assert!(DIGITS.contains(&buf[16]));
    assert!(DIGITS.contains(&buf[17]));
    assert!(DIGITS.contains(&buf[18]));
    assert_eq!(&buf[19..22], &[0, 0, 0]);
    // Version.
    assert_eq!(&buf[22..42], b"1.2.3-alpha1\0\0\0\0\0\0\0\0");
    // Firmware size.
    assert_eq!(buf[42..46], [4, 0, 0, 0]);
    // Public key 1.
    assert_eq!(&buf[46..79], PUBKEY1);
    // Signature 1.
    assert_ne!(&buf[79..143], [0; 64]);
    // Public key 2.
    assert_eq!(&buf[143..176], [0; 33]);
    // Signature 2.
    assert_eq!(buf[176..240], [0; 64]);
    // Reserved.
    assert!(&buf[240..Header::SIZE].iter().all(|&b| b == 0));

    // Add second signature.
    header
        .add_second_signature(&Secp256k1Sign(PUBKEY2))
        .unwrap();

    let mut buf = [0u8; Header::SIZE + 4];
    header.serialize(&mut buf).unwrap();
    buf[Header::SIZE..].copy_from_slice(&firmware);
    let parsed = Header::parse(&buf[..], &[PUBKEY1, PUBKEY2], &Sha256, &Secp256k1Verify)
        .unwrap()
        .unwrap();

    // The parsed header is the same as the serialized header.
    assert_eq!(header.magic(), parsed.magic());
    assert_eq!(header.timestamp(), parsed.timestamp());
    assert_eq!(header.date(), parsed.date());
    assert_eq!(header.version(), parsed.version());
    assert_eq!(header.firmware_size(), parsed.firmware_size());
    assert_eq!(header.pubkey1(), parsed.pubkey1());
    assert_eq!(header.signature1(), parsed.signature1());
    assert_eq!(header.pubkey2(), parsed.pubkey2());
    assert_eq!(header.signature2(), parsed.signature2());

    // The signing procedure filled in the correct fields.
    assert_ne!(header.pubkey1(), [0; 33]);
    assert_ne!(header.signature1(), [0; 64]);
    assert_ne!(header.pubkey2(), [0; 33]);
    assert_ne!(header.signature2(), [0; 64]);

    // The binary representation is correct.
    assert_eq!(buf.len(), Header::SIZE + firmware.len());
    // Magic number.
    assert_eq!(buf[..4], [0x50, 0x52, 0x4D, 0x31]);
    // Timestamp.
    assert_eq!(buf[4..8], header.timestamp().to_le_bytes());
    // Date.
    assert!(MONTHS.contains(&&buf[8..11]));
    assert_eq!(buf[11], b' ');
    assert!(DIGITS.contains(&buf[12]));
    assert!(DIGITS.contains(&buf[13]));
    assert_eq!(buf[14], b' ');
    assert!(DIGITS.contains(&buf[15]));
    assert!(DIGITS.contains(&buf[16]));
    assert!(DIGITS.contains(&buf[17]));
    assert!(DIGITS.contains(&buf[18]));
    assert_eq!(&buf[19..22], &[0, 0, 0]);
    // Version.
    assert_eq!(&buf[22..42], b"1.2.3-alpha1\0\0\0\0\0\0\0\0");
    // Firmware size.
    assert_eq!(buf[42..46], [4, 0, 0, 0]);
    // Public key 1.
    assert_eq!(&buf[46..79], PUBKEY1);
    // Signature 1.
    assert_ne!(&buf[79..143], [0; 64]);
    // Public key 2.
    assert_eq!(&buf[143..176], PUBKEY2);
    // Signature 2.
    assert_ne!(buf[176..240], [0; 64]);
    // Reserved.
    assert!(&buf[240..Header::SIZE].iter().all(|&b| b == 0));
}
