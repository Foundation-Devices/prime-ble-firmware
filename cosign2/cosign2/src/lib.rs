#![no_std]

use {chrono::Datelike, core::str::FromStr};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct Header {
    magic: [u8; 4],
    timestamp: [u8; 4],
    date: [u8; 14],
    version: [u8; 20],
    firmware_size: [u8; 4],
    pubkey1: [u8; 33],
    signature1: [u8; 64],
    pubkey2: [u8; 33],
    signature2: [u8; 64],

    /// Hash of the header and firmware code (not present in the header).
    hash: [u8; 32],
    /// Hash of the firmware only (not present in the header).
    firmware_hash: [u8; 32],
}

/// SHA-256 hash function.
pub trait Sha256 {
    fn hash(&self, data: &[u8]) -> [u8; 32];
}

/// ECDSA secp256k1 signing.
pub trait Secp256k1Sign {
    /// Sign a message on the secp256k1 curve.
    fn sign_ecdsa(&self, msg: [u8; 32]) -> [u8; 64];

    /// Get the public key used for signing.
    fn pubkey(&self) -> [u8; 33];
}

/// ECDSA secp256k1 verification.
pub trait Secp256k1Verify {
    /// Verify an ECDSA signature against the given public key.
    fn verify_ecdsa(
        &self,
        msg: [u8; 32],
        signature: [u8; 64],
        pubkey: [u8; 33],
    ) -> VerificationResult;
}

/// Verification result.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u32)]
pub enum VerificationResult {
    // The values are arbitrary, but chosen to be different by more than one bit to make glitching
    // attacks more difficult.
    Valid = 0xcafebabe,
    Invalid = 0xdeadbeef,
}

impl Header {
    /// Size of the header in bytes.
    pub const SIZE: usize = 2048;

    /// Number of reserved bytes at the end of the header.
    pub const RESERVED: usize = 1808;

    /// Magic number.
    pub fn magic(&self) -> Magic {
        Magic::from_bytes(self.magic).expect("validated")
    }

    /// Firmware timestamp in seconds since the Unix epoch.
    pub fn timestamp(&self) -> u32 {
        u32::from_le_bytes(self.timestamp)
    }

    /// Human-readable firmware date.
    ///
    /// Used for displaying the firmware date during device boot, where advanced
    /// date manipulation functions are not available.
    pub fn date(&self) -> &str {
        let first_zero = self
            .date
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.date.len());
        core::str::from_utf8(&self.date[..first_zero]).expect("validated")
    }

    /// Firmware version.
    pub fn version(&self) -> &str {
        let first_zero = self
            .version
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.version.len());
        core::str::from_utf8(&self.version[..first_zero]).expect("validated")
    }

    /// The size of the firmware.
    pub fn firmware_size(&self) -> u32 {
        u32::from_le_bytes(self.firmware_size)
    }

    /// Public key of the first signer.
    pub fn pubkey1(&self) -> [u8; 33] {
        self.pubkey1
    }

    /// First signature.
    pub fn signature1(&self) -> [u8; 64] {
        self.signature1
    }

    /// Public key of the second signer.
    pub fn pubkey2(&self) -> [u8; 33] {
        self.pubkey2
    }

    /// Second signature.
    pub fn signature2(&self) -> [u8; 64] {
        self.signature2
    }

    /// Hash of the firmware that this header is for.
    ///
    /// All zeros if the header was created using
    /// [`parse_unverified`](Header::parse_unverified).
    pub fn firmware_hash(&self) -> &[u8; 32] {
        &self.firmware_hash
    }

    /// Create a new header and sign it.
    pub fn sign_new(
        magic: Magic,
        version: &str,
        timestamp: u32,
        signer: Signer,
        firmware: &[u8],
        sha: &impl Sha256,
        secp: &impl Secp256k1Sign,
    ) -> Result<Self, Error> {
        // Validate the version string.
        #[cfg(feature = "semver")]
        semver::Version::from_str(version).map_err(|_| Error::InvalidVersionSemver)?;

        let mut header = Self {
            magic: magic.to_bytes(),
            timestamp: timestamp.to_le_bytes(),
            date: [0; 14],
            version: [0; 20],
            firmware_size: u32::try_from(firmware.len())
                .map_err(|_| Error::FirmwareTooLong)?
                .to_le_bytes(),
            pubkey1: [0; 33],
            signature1: [0; 64],
            pubkey2: [0; 33],
            signature2: [0; 64],
            hash: [0; 32],
            firmware_hash: [0; 32],
        };
        header.set_date(timestamp);
        header.set_version(version)?;
        header.hash(&[0; Self::RESERVED], firmware, sha);
        header.validate_fields(firmware)?;

        // Sign the header.
        match signer {
            Signer::Trusted => {
                // Trusted key is used, so both first and second signatures need to be filled
                // in. This key is used for the first signature.
                header.pubkey1 = secp.pubkey();
                header.signature1 = secp.sign_ecdsa(header.hash);
            }
            Signer::Developer => {
                // Developer key is used, so only the second signature is filled in.
                // The first signature is left empty (zeroed out).
                header.pubkey2 = secp.pubkey();
                header.signature2 = secp.sign_ecdsa(header.hash);
            }
        }

        Ok(header)
    }

    /// Parse a header.
    ///
    /// Verifies that any existing signatures are signed by the given known
    /// signers. If the known signers slice is empty, any signer is
    /// accepted.
    ///
    /// If the header is missing, `None` is returned.
    pub fn parse(
        data: &[u8],
        known_signers: &[[u8; 33]],
        sha: &impl Sha256,
        secp: &impl Secp256k1Verify,
    ) -> Result<Option<Self>, Error> {
        let Some(mut header) = Header::deserialize(data)? else {
            return Ok(None);
        };

        // When parsing an existing header, there should always be at least one
        // signature.
        if header.signature1 == [0; 64] && header.signature2 == [0; 64] {
            return Err(Error::HeaderWithNoSignature);
        }

        let reserved = &data[240..Self::SIZE];
        let firmware = &data[Self::SIZE..];
        header.hash(reserved, firmware, sha);
        header.verify_signatures(known_signers, secp)?;
        header.validate_fields(firmware)?;

        // Check that the reserved bytes are all zero.
        if reserved.iter().any(|&b| b != 0) {
            return Err(Error::InvalidReservedBytes);
        }

        Ok(Some(header))
    }

    /// Reads the header without verifying the signatures. Be careful with
    /// trusting the unverified data.
    ///
    /// Use the [`parse`](Header::parse) method to read and verify the header
    /// signatures.
    pub fn parse_unverified(data: &[u8]) -> Result<Option<Self>, Error> {
        let Some(header) = Self::deserialize(data)? else {
            return Ok(None);
        };
        header.validate_fields(&data[Self::SIZE..])?;
        Ok(Some(header))
    }

    /// Serialize the header to a buffer. Exactly [`Self::SIZE`] bytes will be
    /// written.
    pub fn serialize(&self, buf: &mut [u8]) -> Result<(), Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::SerializeBufferTooSmall);
        }

        buf[..4].copy_from_slice(&self.magic);
        buf[4..8].copy_from_slice(&self.timestamp);
        buf[8..22].copy_from_slice(&self.date);
        buf[22..42].copy_from_slice(&self.version);
        buf[42..46].copy_from_slice(&self.firmware_size);
        buf[46..79].copy_from_slice(&self.pubkey1);
        buf[79..143].copy_from_slice(&self.signature1);
        buf[143..176].copy_from_slice(&self.pubkey2);
        buf[176..240].copy_from_slice(&self.signature2);
        buf[240..Self::SIZE].copy_from_slice(&[0u8; Self::RESERVED]);

        Ok(())
    }

    /// Add a second signature to the header.
    ///
    /// The first signature must be present, and the second signature must be
    /// missing, otherwise an error is returned.
    pub fn add_second_signature(&mut self, secp: &impl Secp256k1Sign) -> Result<(), Error> {
        if self.signature2 != [0; 64] {
            return Err(Error::Signature2Present);
        }
        if self.signature1 == [0; 64] {
            return Err(Error::Signature1Missing);
        }
        let pubkey = secp.pubkey();
        if self.pubkey1 == pubkey {
            return Err(Error::PubkeyAlreadyUsed);
        }
        self.pubkey2 = pubkey;
        self.signature2 = secp.sign_ecdsa(self.hash);
        Ok(())
    }

    /// Deserialize the header fields from a buffer.
    ///
    /// Returns `None` if the buffer does not contain a header.
    fn deserialize(data: &[u8]) -> Result<Option<Self>, Error> {
        if data.len() < 4 {
            return Ok(None);
        }

        let magic = data[..4].try_into().unwrap();
        if Magic::from_bytes(magic).is_none() {
            // Magic value is missing or not recognized, so this is not a header.
            return Ok(None);
        }

        // The data contains a header, so make sure it's of appropriate length.
        if data.len() < Self::SIZE {
            return Err(Error::HeaderTooShort);
        }

        let timestamp = data[4..8].try_into().unwrap();
        let date = data[8..22].try_into().unwrap();
        let version = data[22..42].try_into().unwrap();
        let firmware_size = data[42..46].try_into().unwrap();
        let pubkey1 = data[46..79].try_into().unwrap();
        let signature1 = data[79..143].try_into().unwrap();
        let pubkey2 = data[143..176].try_into().unwrap();
        let signature2 = data[176..240].try_into().unwrap();

        Ok(Some(Self {
            magic,
            timestamp,
            date,
            version,
            firmware_size,
            pubkey1,
            signature1,
            pubkey2,
            signature2,
            hash: [0; 32],
            firmware_hash: [0; 32],
        }))
    }

    /// Hash up the header and store it in the `hash` field.
    fn hash(&mut self, reserved: &[u8], firmware: &[u8], sha: &impl Sha256) {
        let mut hash_buf = [0; 128];

        // Fill header hash buf with header data
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

        let header_hash = sha.hash(&hash_buf);
        let reserved_hash = sha.hash(reserved);
        let firmware_hash = sha.hash(firmware);

        hash_buf.fill(0);
        hash_buf[0..32].copy_from_slice(&header_hash);
        hash_buf[32..64].copy_from_slice(&reserved_hash);
        hash_buf[64..96].copy_from_slice(&firmware_hash);
        let hash = sha.hash(&hash_buf);

        // Hash twice to prevent length extension attacks
        self.hash = sha.hash(&hash);
        self.firmware_hash = firmware_hash;
    }

    fn verify_signatures(
        &mut self,
        known_signers: &[[u8; 33]],
        secp: &impl Secp256k1Verify,
    ) -> Result<(), Error> {
        if self.signature1 != [0; 64] {
            if !known_signers.is_empty() && !known_signers.contains(&self.pubkey1) {
                return Err(Error::UnknownPubkey1);
            }
            if secp.verify_ecdsa(self.hash, self.signature1, self.pubkey1)
                != VerificationResult::Valid
            {
                return Err(Error::InvalidSignature1);
            }
        }
        if self.signature2 != [0; 64] {
            if !known_signers.is_empty() && !known_signers.contains(&self.pubkey2) {
                return Err(Error::UnknownPubkey2);
            }
            if secp.verify_ecdsa(self.hash, self.signature2, self.pubkey2)
                != VerificationResult::Valid
            {
                return Err(Error::InvalidSignature2);
            }
        }
        Ok(())
    }

    /// Validate the fields in the header.
    fn validate_fields(&self, firmware: &[u8]) -> Result<(), Error> {
        // Validate that the version string is UTF-8 formatted according to SemVer, and
        // that the unused bytes are all zero.
        let first_zero = self
            .version
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.version.len());
        let version = core::str::from_utf8(&self.version[..first_zero])
            .map_err(|_| Error::InvalidVersionUtf8)?;
        #[cfg(feature = "semver")]
        semver::Version::from_str(version).map_err(|_| Error::InvalidVersionSemver)?;
        if self.version[first_zero..].iter().any(|&b| b != 0) {
            return Err(Error::InvalidVersionTrailingBytes);
        }

        // Validate that the date string is UTF-8, and that the unused bytes are all
        // zero.
        let first_zero = self
            .date
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.date.len());
        core::str::from_utf8(&self.date[..first_zero]).map_err(|_| Error::InvalidDateUtf8)?;
        if self.date[first_zero..].iter().any(|&b| b != 0) {
            return Err(Error::InvalidDateTrailingBytes);
        }

        // Verify that the firmware size is correct.
        let firmware_size = u32::from_le_bytes(self.firmware_size);
        let actual_firmware_size =
            u32::try_from(firmware.len()).map_err(|_| Error::FirmwareTooLong)?;
        if firmware_size != actual_firmware_size {
            return Err(Error::InvalidFirmwareSize {
                header: firmware_size,
                actual: actual_firmware_size,
            });
        }

        // If a signature is zero, the corresponding pubkey must also be zero.
        if self.signature1 == [0; 64] && self.pubkey1 != [0; 33] {
            return Err(Error::InvalidPubkey1);
        }
        if self.signature2 == [0; 64] && self.pubkey2 != [0; 33] {
            return Err(Error::InvalidPubkey2);
        }

        Ok(())
    }

    /// Set the firmware date field.
    fn set_date(&mut self, timestamp: u32) {
        let date = chrono::DateTime::from_timestamp(timestamp.into(), 0).expect("before 2106");
        let month = match date.month() {
            1 => b"Jan",
            2 => b"Feb",
            3 => b"Mar",
            4 => b"Apr",
            5 => b"May",
            6 => b"Jun",
            7 => b"Jul",
            8 => b"Aug",
            9 => b"Sep",
            10 => b"Oct",
            11 => b"Nov",
            12 => b"Dec",
            _ => unreachable!(),
        };
        self.date[0..3].copy_from_slice(month);
        self.date[3] = b' ';
        let day = date.day();
        self.date[4] = ascii_digit(day / 10);
        self.date[5] = ascii_digit(day % 10);
        self.date[6] = b' ';
        let year: u32 = date.year().try_into().expect("year in timestamp is valid");
        self.date[7] = ascii_digit((year / 1000) % 10);
        self.date[8] = ascii_digit((year / 100) % 10);
        self.date[9] = ascii_digit((year / 10) % 10);
        self.date[10] = ascii_digit(year % 10);
    }

    /// Set the firmware version field.
    fn set_version(&mut self, version: &str) -> Result<(), Error> {
        if version.len() > self.version.len() {
            return Err(Error::VersionTooLong);
        }
        self.version[..version.len()].copy_from_slice(version.as_bytes());
        Ok(())
    }
}

/// Who is signing the firmware.
pub enum Signer {
    /// Signed by a trusted identity from Foundation Devices, Inc.
    ///
    /// Headers signed by trusted keys expect both signatures to be filled in.
    Trusted,
    /// Signed by a third-party developer.
    ///
    /// Headers signed by developer keys expect only the second signature to be
    /// filled in. The first signature is left empty (zeroed out).
    Developer,
}

/// Magic number.
///
/// Used to identify the header, and to differentiate header formats for
/// different devices if needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Magic {
    Atsama5d27KeyOs,
}

impl Magic {
    pub fn from_bytes(b: [u8; 4]) -> Option<Self> {
        match b {
            [0x50, 0x52, 0x4D, 0x31] => Some(Self::Atsama5d27KeyOs),
            _ => None,
        }
    }

    pub fn to_bytes(&self) -> [u8; 4] {
        match self {
            Self::Atsama5d27KeyOs => [0x50, 0x52, 0x4D, 0x31],
        }
    }
}

fn ascii_digit(b: u32) -> u8 {
    match b {
        0 => b'0',
        1 => b'1',
        2 => b'2',
        3 => b'3',
        4 => b'4',
        5 => b'5',
        6 => b'6',
        7 => b'7',
        8 => b'8',
        9 => b'9',
        _ => unreachable!(),
    }
}

#[derive(Debug)]
pub enum Error {
    FirmwareTooLong,
    HeaderTooShort,
    HeaderWithNoSignature,
    InvalidDateTrailingBytes,
    InvalidDateUtf8,
    InvalidFirmwareSize { header: u32, actual: u32 },
    InvalidPubkey1,
    InvalidPubkey2,
    InvalidReservedBytes,
    InvalidSignature1,
    InvalidSignature2,
    InvalidTimestamp,
    InvalidVersionSemver,
    InvalidVersionTrailingBytes,
    InvalidVersionUtf8,
    PubkeyAlreadyUsed,
    SerializeBufferTooSmall,
    Signature1Missing,
    Signature2Present,
    UnknownPubkey1,
    UnknownPubkey2,
    VersionTooLong,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FirmwareTooLong => write!(f, "firmware too long"),
            Self::HeaderTooShort => write!(f, "header too short"),
            Self::HeaderWithNoSignature => write!(f, "header with no signature"),
            Self::InvalidDateTrailingBytes => write!(f, "invalid date trailing bytes in header"),
            Self::InvalidDateUtf8 => write!(f, "invalid date UTF-8 in header"),
            Self::InvalidFirmwareSize {
                header: in_header,
                actual,
            } => write!(
                f,
                "invalid firmware size in header: should be {actual}, but is {in_header}",
            ),
            Self::InvalidPubkey1 => write!(f, "invalid pubkey1 in header"),
            Self::InvalidPubkey2 => write!(f, "invalid pubkey2 in header"),
            Self::InvalidReservedBytes => write!(f, "invalid reserved bytes in header"),
            Self::InvalidSignature1 => write!(f, "invalid signature1 in header"),
            Self::InvalidSignature2 => write!(f, "invalid signature2 in header"),
            Self::InvalidTimestamp => write!(f, "invalid timestamp in header"),
            Self::InvalidVersionSemver => write!(f, "invalid version SemVer in header"),
            Self::InvalidVersionTrailingBytes => {
                write!(f, "invalid version trailing bytes in header")
            }
            Self::InvalidVersionUtf8 => write!(f, "invalid version UTF-8 in header"),
            Self::PubkeyAlreadyUsed => write!(f, "attempting to sign with the same pubkey twice"),
            Self::SerializeBufferTooSmall => write!(f, "buffer too small for serialization"),
            Self::Signature1Missing => write!(f, "signature1 missing in header"),
            Self::Signature2Present => {
                write!(f, "signature2 already present in header")
            }
            Self::UnknownPubkey1 => write!(f, "unknown pubkey1 in header"),
            Self::UnknownPubkey2 => write!(f, "unknown pubkey2 in header"),
            Self::VersionTooLong => write!(f, "version too long to write in header"),
        }
    }
}
