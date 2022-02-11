use std::fmt;

use hex::ToHex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// TODO Use base64 representation instead

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Id(#[serde(with = "hex")] [u8; 32]);

impl Id {
    pub fn of(str: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(str);
        Self(hasher.finalize().into())
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.encode_hex::<String>())
    }
}
