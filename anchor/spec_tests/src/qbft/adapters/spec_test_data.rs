use sha2::{Digest, Sha256};
use ssv_types::consensus::QbftData;
use ssz_derive::{Decode, Encode};
use types::Hash256;

/// Simple test data wrapper for QBFT spec tests
/// Just holds raw bytes that can be SSZ encoded/decoded
#[derive(Debug, Clone, Encode, Decode, PartialEq)]
pub struct SpecTestData {
    pub data: Vec<u8>,
}

impl SpecTestData {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    pub fn from_base64(s: &str) -> Result<Self, base64::DecodeError> {
        let data = base64::decode(s)?;
        Ok(Self { data })
    }
}

impl QbftData for SpecTestData {
    type Hash = Hash256;

    fn hash(&self) -> Self::Hash {
        let mut hasher = Sha256::new();
        hasher.update(&self.data);
        Hash256::from_slice(&hasher.finalize())
    }

    fn validate(&self) -> bool {
        // For test data, always valid
        true
    }
}

impl Default for SpecTestData {
    fn default() -> Self {
        Self { data: vec![] }
    }
}
