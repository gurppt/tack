use crate::{Error, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::{fmt, io::Read};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WireId(u128);
impl WireId {
    pub fn new(value: u128) -> Result<Self> {
        if value == 0 {
            Err(Error::Invalid("zero identity"))
        } else {
            Ok(Self(value))
        }
    }
    pub fn value(self) -> u128 {
        self.0
    }
    pub fn parse(value: &str) -> Result<Self> {
        if value.len() != 32 || !value.bytes().all(hex_digit) {
            return Err(Error::Invalid("identity encoding"));
        }
        Self::new(u128::from_str_radix(value, 16).map_err(|_| Error::Invalid("identity encoding"))?)
    }
}
impl fmt::Display for WireId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}
impl Serialize for WireId {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for WireId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash(String);
impl ContentHash {
    pub fn parse(value: &str) -> Result<Self> {
        if value.len() != 64 || !value.bytes().all(hex_digit) {
            return Err(Error::Invalid("SHA256 encoding"));
        }
        Ok(Self(value.to_owned()))
    }
    pub fn digest(bytes: &[u8]) -> Self {
        Self(encode_hex(&Sha256::digest(bytes)))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

pub fn hash_reader(reader: &mut impl Read) -> Result<(ContentHash, u64)> {
    let mut digest = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        size = size
            .checked_add(n as u64)
            .ok_or(Error::Invalid("asset length overflow"))?;
        if size > crate::MAX_ASSET_BYTES {
            return Err(Error::Invalid("asset size budget"));
        }
        digest.update(&buffer[..n]);
    }
    Ok((ContentHash(encode_hex(&digest.finalize())), size))
}
fn hex_digit(b: u8) -> bool {
    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
}
pub fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for &b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 15)]));
    }
    out
}
pub fn decode_hex(value: &str, maximum: usize) -> Result<Vec<u8>> {
    if !value.len().is_multiple_of(2) || value.len() / 2 > maximum || !value.bytes().all(hex_digit)
    {
        return Err(Error::Invalid("hex payload length/encoding"));
    }
    let mut out = Vec::with_capacity(value.len() / 2);
    fn digit(b: u8) -> u8 {
        if b <= b'9' { b - b'0' } else { b - b'a' + 10 }
    }
    for pair in value.as_bytes().chunks_exact(2) {
        out.push((digit(pair[0]) << 4) | digit(pair[1]));
    }
    Ok(out)
}
