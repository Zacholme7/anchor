use sha2::{Digest, Sha256};
use ssv_types::consensus::BeaconVote;
use types::{Checkpoint, Hash256};

/// Create a BeaconVote from test data bytes or hash
pub fn create_beacon_vote_from_bytes(data: &[u8]) -> BeaconVote {
    // If data is 32 bytes, use it directly as hash, otherwise hash it
    let hash = if data.len() == 32 {
        Hash256::from_slice(data)
    } else {
        hash_data(data)
    };

    BeaconVote {
        block_root: hash,
        source: Checkpoint {
            epoch: types::Epoch::new(0),
            root: hash,
        },
        target: Checkpoint {
            epoch: types::Epoch::new(1),
            root: hash,
        },
    }
}

/// Hash data using SHA256 and return as Hash256
pub fn hash_data(data: &[u8]) -> Hash256 {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash_bytes: [u8; 32] = hasher.finalize().into();
    Hash256::from(hash_bytes)
}

/// Calculate QBFT quorum size for a committee
/// Formula: quorum = n - f where f = (n-1)/3
pub fn calculate_quorum(committee_size: usize) -> usize {
    let f = (committee_size - 1) / 3;
    committee_size - f
}
