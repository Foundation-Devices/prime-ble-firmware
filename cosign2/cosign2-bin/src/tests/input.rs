//! Creating input files to run tests on.

use {
    sha2::{Digest, Sha256},
    std::io::Write,
};

pub const SECRET_1_PEM: &[u8] = br"-----BEGIN EC PRIVATE KEY-----
MHQCAQEEILP3haO4tNGh6X7CeC0a+o9Ad2WVrHaFieIYxXKogc+7oAcGBSuBBAAK
oUQDQgAEcaPmBleCgfa8tGGoraZxgpwzSLaIeHxouqda1RkW7fwLSTPLAocNwtdM
cg9lrtvVxKhQfKOnbLtoZxdC82sJJA==
-----END EC PRIVATE KEY-----";
pub const SECRET_2_PEM: &[u8] = br"-----BEGIN EC PRIVATE KEY-----
MHQCAQEEIKWt4HaQLfMPkpyZOO0Nn9BC0CSPp/D3cyIBLNob3Xv0oAcGBSuBBAAK
oUQDQgAEEY9ZCCuOJnU8HKsb/q0yWTRMAzEsu0/3vezOuRXpLJu1JKvtgYI/b5PR
cL6iEfU7G/wu1pITiXhOX88QGg37qA==
-----END EC PRIVATE KEY-----";

pub const PUBKEY_1_BYTES: [u8; 33] = [
    2, 113, 163, 230, 6, 87, 130, 129, 246, 188, 180, 97, 168, 173, 166, 113, 130, 156, 51, 72,
    182, 136, 120, 124, 104, 186, 167, 90, 213, 25, 22, 237, 252,
];
pub const PUBKEY_2_BYTES: [u8; 33] = [
    2, 17, 143, 89, 8, 43, 142, 38, 117, 60, 28, 171, 27, 254, 173, 50, 89, 52, 76, 3, 49, 44, 187,
    79, 247, 189, 236, 206, 185, 21, 233, 44, 155,
];

pub const PUBKEY_1_HEX: &str = "0271a3e606578281f6bcb461a8ada671829c3348b688787c68baa75ad51916edfc";
pub const PUBKEY_2_HEX: &str = "02118f59082b8e26753c1cab1bfead3259344c03312cbb4ff7bdecceb915e92c9b";

pub const SECRET_1_BYTES: &[u8] = &[
    179, 247, 133, 163, 184, 180, 209, 161, 233, 126, 194, 120, 45, 26, 250, 143, 64, 119, 101,
    149, 172, 118, 133, 137, 226, 24, 197, 114, 168, 129, 207, 187,
];
pub const SECRET_2_BYTES: &[u8] = &[
    165, 173, 224, 118, 144, 45, 243, 15, 146, 156, 153, 56, 237, 13, 159, 208, 66, 208, 36, 143,
    167, 240, 247, 115, 34, 1, 44, 218, 27, 221, 123, 244,
];

#[derive(Debug, Clone)]
pub struct Params {
    pub magic: bool,
    pub timestamp: u32,
    pub date: Vec<u8>,
    pub version: Vec<u8>,
    pub signature1: bool,
    pub signature2: bool,
    pub reserved: Vec<u8>,
    pub firmware: Vec<u8>,
    pub firmware_size: u32,
}

#[derive(Debug, Clone)]
pub struct Input {
    pub magic: [u8; 4],
    pub timestamp: [u8; 4],
    pub date: [u8; 14],
    pub version: [u8; 20],
    pub firmware_size: [u8; 4],
    pub pubkey1: [u8; 33],
    pub signature1: [u8; 64],
    pub pubkey2: [u8; 33],
    pub signature2: [u8; 64],
    pub reserved: [u8; cosign2::Header::RESERVED],
    pub firmware: Vec<u8>,
}

impl Params {
    pub fn input(mut self) -> Input {
        self.date.resize(14, 0);
        self.version.resize(20, 0);
        self.reserved.resize(cosign2::Header::RESERVED, 0);
        let mut input = Input {
            magic: if self.magic {
                [0x50, 0x52, 0x4D, 0x31]
            } else {
                [0x12, 0x34, 0x56, 0x78]
            },
            timestamp: self.timestamp.to_le_bytes(),
            date: self.date.as_slice().try_into().unwrap(),
            version: self.version.as_slice().try_into().unwrap(),
            firmware_size: self.firmware_size.to_le_bytes(),
            pubkey1: if self.signature1 {
                PUBKEY_1_BYTES
            } else {
                [0; 33]
            },
            signature1: [0; 64],
            pubkey2: if self.signature2 {
                PUBKEY_2_BYTES
            } else {
                [0; 33]
            },
            signature2: [0; 64],
            reserved: self.reserved.as_slice().try_into().unwrap(),
            firmware: self.firmware,
        };
        input.sign();
        input
    }
}

impl Input {
    /// Update the input with a closure.
    pub fn update(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }

    /// Write the input to a file. Return the file and the two signatures
    /// encoded in hex.
    pub fn create_file(self) -> (tempfile::NamedTempFile, String, String) {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&self.magic).unwrap();
        file.write_all(&self.timestamp).unwrap();
        file.write_all(&self.date).unwrap();
        file.write_all(&self.version).unwrap();
        file.write_all(&self.firmware_size).unwrap();
        file.write_all(&self.pubkey1).unwrap();
        file.write_all(&self.signature1).unwrap();
        file.write_all(&self.pubkey2).unwrap();
        file.write_all(&self.signature2).unwrap();
        file.write_all(&self.reserved).unwrap();
        file.write_all(&self.firmware).unwrap();
        (
            file,
            hex::encode(self.signature1),
            hex::encode(self.signature2),
        )
    }

    fn sign(&mut self) {
        let mut hash_buf = [0; 128];

        let mut offset = 0;
        hash_buf[offset..offset + self.magic.len()].copy_from_slice(&self.magic);
        offset += self.magic.len();
        hash_buf[offset..offset + self.timestamp.len()].copy_from_slice(&self.timestamp);
        offset += self.timestamp.len();
        hash_buf[offset..offset + self.date.len()].copy_from_slice(&self.date);
        offset += self.date.len();
        hash_buf[offset..offset + self.version.len()].copy_from_slice(&self.version);
        offset += self.version.len();
        hash_buf[offset..offset + self.firmware_size.len()].copy_from_slice(&self.firmware_size);

        let header_hash = Sha256::digest(hash_buf);
        let reserved_hash = Sha256::digest(self.reserved);
        let firmware_hash = Sha256::digest(&self.firmware);

        hash_buf.fill(0);
        hash_buf[0..32].copy_from_slice(&header_hash);
        hash_buf[32..64].copy_from_slice(&reserved_hash);
        hash_buf[64..96].copy_from_slice(&firmware_hash);

        let hash = Sha256::digest(hash_buf);
        let hash = Sha256::digest(hash);

        let secp = secp256k1::Secp256k1::new();
        if self.pubkey1 != [0; 33] {
            let secret_key = secp256k1::SecretKey::from_slice(SECRET_1_BYTES).unwrap();
            let signature =
                secp.sign_ecdsa(&secp256k1::Message::from_digest(hash.into()), &secret_key);
            self.signature1 = signature.serialize_compact();
        }
        if self.pubkey2 != [0; 33] {
            let secret_key = secp256k1::SecretKey::from_slice(SECRET_2_BYTES).unwrap();
            let signature =
                secp.sign_ecdsa(&secp256k1::Message::from_digest(hash.into()), &secret_key);
            self.signature2 = signature.serialize_compact();
        }
    }
}
