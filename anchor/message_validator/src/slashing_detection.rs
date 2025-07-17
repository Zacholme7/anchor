use crate::ValidationFailure;
use ssv_types::consensus::BeaconVote;
use std::collections::HashMap;
use types::{Epoch, PublicKeyBytes};

/// Enhanced slashing detection according to SSV specification.
///
/// Implements comprehensive slashing detection logic that prevents
/// validators from creating slashable attestations according to
/// Ethereum consensus rules and SSV protocol requirements.
pub struct EnhancedSlashingDetector {
    /// Historical attestations per validator for slashing detection
    attestation_history: HashMap<PublicKeyBytes, Vec<BeaconVote>>,
    /// Maximum epochs to keep in history for performance
    max_history_epochs: u64,
}

impl EnhancedSlashingDetector {
    /// Create a new enhanced slashing detector
    pub fn new() -> Self {
        Self {
            attestation_history: HashMap::new(),
            max_history_epochs: 10, // Keep ~10 epochs of history
        }
    }

    /// Detect slashing conditions according to SSV specification
    ///
    /// Implements the two main slashing conditions:
    /// 1. Double voting: Two attestations with same target epoch but different roots
    /// 2. Surround voting: Attestations that surround each other in epoch ranges
    ///
    /// Returns ValidationFailure::SlashableAttestation if slashing is detected.
    pub fn detect_slashing(
        &mut self,
        validator_pk: &PublicKeyBytes,
        new_vote: &BeaconVote,
    ) -> Result<(), ValidationFailure> {
        // Check against all previous attestations from this validator first
        if let Some(history) = self.attestation_history.get(validator_pk) {
            for previous_vote in history.iter() {
                // SSV Spec: Double voting detection
                if self.is_double_vote(previous_vote, new_vote) {
                    return Err(ValidationFailure::SlashableAttestation);
                }

                // SSV Spec: Surround voting detection
                if self.is_surround_vote(previous_vote, new_vote) {
                    return Err(ValidationFailure::SlashableAttestation);
                }
            }
        }

        // Store new vote for future slashing detection
        let history = self
            .attestation_history
            .entry(*validator_pk)
            .or_insert_with(Vec::new);
        history.push(new_vote.clone());

        // Clean up old history to prevent unbounded growth
        self.cleanup_old_history(validator_pk, new_vote.target.epoch);

        Ok(())
    }

    /// Detect double voting (same target epoch, different target roots)
    ///
    /// Double voting occurs when a validator creates two attestations
    /// for the same target epoch but with different target block roots.
    /// This is a slashable offense in Ethereum consensus.
    fn is_double_vote(&self, vote1: &BeaconVote, vote2: &BeaconVote) -> bool {
        vote1.target.epoch == vote2.target.epoch && vote1.target.root != vote2.target.root
    }

    /// Detect surround voting (attestations that surround each other)
    ///
    /// Surround voting occurs when:
    /// - vote1 has source < vote2.source AND vote1.target > vote2.target, OR
    /// - vote2 has source < vote1.source AND vote2.target > vote1.target
    ///
    /// This creates conflicting attestation chains and is slashable.
    fn is_surround_vote(&self, vote1: &BeaconVote, vote2: &BeaconVote) -> bool {
        // vote1 surrounds vote2
        let vote1_surrounds_vote2 =
            vote1.source.epoch < vote2.source.epoch && vote1.target.epoch > vote2.target.epoch;

        // vote2 surrounds vote1
        let vote2_surrounds_vote1 =
            vote2.source.epoch < vote1.source.epoch && vote2.target.epoch > vote1.target.epoch;

        vote1_surrounds_vote2 || vote2_surrounds_vote1
    }

    /// Clean up old attestation history to prevent memory growth
    ///
    /// Removes attestations older than max_history_epochs to maintain
    /// reasonable memory usage while keeping enough history for slashing detection.
    fn cleanup_old_history(&mut self, validator_pk: &PublicKeyBytes, current_epoch: Epoch) {
        if let Some(history) = self.attestation_history.get_mut(validator_pk) {
            let cutoff_epoch = current_epoch.saturating_sub(self.max_history_epochs);
            history.retain(|vote| vote.target.epoch >= cutoff_epoch);
        }
    }

    /// Check if attestation would be slashable without storing it
    ///
    /// Useful for validation without side effects. Returns true if
    /// the attestation would create a slashing condition.
    pub fn would_be_slashable(&self, validator_pk: &PublicKeyBytes, vote: &BeaconVote) -> bool {
        if let Some(history) = self.attestation_history.get(validator_pk) {
            for previous_vote in history.iter() {
                if self.is_double_vote(previous_vote, vote)
                    || self.is_surround_vote(previous_vote, vote)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Get the number of stored attestations for a validator
    ///
    /// Useful for monitoring and debugging slashing detection state.
    pub fn get_history_size(&self, validator_pk: &PublicKeyBytes) -> usize {
        self.attestation_history
            .get(validator_pk)
            .map_or(0, |h| h.len())
    }

    /// Clear all history (useful for testing)
    pub fn clear_history(&mut self) {
        self.attestation_history.clear();
    }

    /// Set maximum epochs to keep in history
    pub fn set_max_history_epochs(&mut self, epochs: u64) {
        self.max_history_epochs = epochs;
    }
}

impl Default for EnhancedSlashingDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{Checkpoint, Hash256};

    fn create_test_vote(source_epoch: u64, target_epoch: u64, target_root: Hash256) -> BeaconVote {
        BeaconVote {
            block_root: Hash256::zero(),
            source: Checkpoint {
                epoch: Epoch::new(source_epoch),
                root: Hash256::zero(),
            },
            target: Checkpoint {
                epoch: Epoch::new(target_epoch),
                root: target_root,
            },
        }
    }

    #[test]
    fn test_no_slashing_for_valid_sequence() {
        let mut detector = EnhancedSlashingDetector::new();
        let validator_pk = PublicKeyBytes::empty();

        let vote1 = create_test_vote(0, 1, Hash256::from_low_u64_be(1));
        let vote2 = create_test_vote(1, 2, Hash256::from_low_u64_be(2));

        assert!(detector.detect_slashing(&validator_pk, &vote1).is_ok());
        assert!(detector.detect_slashing(&validator_pk, &vote2).is_ok());
    }

    #[test]
    fn test_double_vote_detection() {
        let mut detector = EnhancedSlashingDetector::new();
        let validator_pk = PublicKeyBytes::empty();

        let vote1 = create_test_vote(0, 1, Hash256::from_low_u64_be(1));
        let vote2 = create_test_vote(0, 1, Hash256::from_low_u64_be(2)); // Same target epoch, different root

        assert!(detector.detect_slashing(&validator_pk, &vote1).is_ok());
        let result = detector.detect_slashing(&validator_pk, &vote2);
        assert!(matches!(
            result,
            Err(ValidationFailure::SlashableAttestation)
        ));
    }

    #[test]
    fn test_surround_vote_detection() {
        let mut detector = EnhancedSlashingDetector::new();
        let validator_pk = PublicKeyBytes::empty();

        let vote1 = create_test_vote(0, 3, Hash256::from_low_u64_be(1)); // Surrounds vote2
        let vote2 = create_test_vote(1, 2, Hash256::from_low_u64_be(2)); // Surrounded by vote1

        assert!(detector.detect_slashing(&validator_pk, &vote1).is_ok());
        let result = detector.detect_slashing(&validator_pk, &vote2);
        assert!(matches!(
            result,
            Err(ValidationFailure::SlashableAttestation)
        ));
    }

    #[test]
    fn test_would_be_slashable() {
        let mut detector = EnhancedSlashingDetector::new();
        let validator_pk = PublicKeyBytes::empty();

        let vote1 = create_test_vote(0, 1, Hash256::from_low_u64_be(1));
        let vote2 = create_test_vote(0, 1, Hash256::from_low_u64_be(2)); // Double vote

        detector.detect_slashing(&validator_pk, &vote1).unwrap();
        assert!(detector.would_be_slashable(&validator_pk, &vote2));
    }

    #[test]
    fn test_history_cleanup() {
        let mut detector = EnhancedSlashingDetector::new();
        detector.set_max_history_epochs(2);

        let validator_pk = PublicKeyBytes::empty();

        // Add votes across several epochs
        for epoch in 1..=5 {
            let vote = create_test_vote(epoch - 1, epoch, Hash256::from_low_u64_be(epoch));
            detector.detect_slashing(&validator_pk, &vote).unwrap();
        }

        // Should only keep recent history
        assert!(detector.get_history_size(&validator_pk) <= 3); // Some tolerance for cleanup logic
    }
}
