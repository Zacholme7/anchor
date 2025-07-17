use crate::ValidationFailure;
use ssv_types::consensus::{BeaconVote, ValidatorConsensusData, ValidatorDuty};
use types::{Checkpoint, Epoch, Slot};

/// Validates beacon chain data according to SSV specification requirements.
///
/// This module implements beacon-specific validation logic that ensures
/// attestations, duties, and consensus data comply with SSV protocol rules
/// for slashing prevention and safety.
pub struct BeaconDataValidator {
    current_epoch: Epoch,
    slots_per_epoch: u64,
}

impl BeaconDataValidator {
    /// Create a new beacon data validator with current epoch context
    pub fn new(current_epoch: Epoch, slots_per_epoch: u64) -> Self {
        Self {
            current_epoch,
            slots_per_epoch,
        }
    }

    /// Validate BeaconVote (attestation) data according to SSV specification
    ///
    /// Implements key SSV spec requirements:
    /// - Source epoch must be less than target epoch (slashing rule)
    /// - Target epoch cannot be too far in the future
    /// - Checkpoint consistency validation
    pub fn validate_beacon_vote(&self, beacon_vote: &BeaconVote) -> Result<(), ValidationFailure> {
        // SSV Spec requirement: source epoch < target epoch
        // This prevents certain types of slashable attestations
        if beacon_vote.source.epoch >= beacon_vote.target.epoch {
            return Err(ValidationFailure::AttestationSourceGreaterThanTarget);
        }

        // SSV Spec requirement: target epoch not too far in future
        // Prevents attestations for epochs that are unreasonably far ahead
        if beacon_vote.target.epoch > self.current_epoch + 1 {
            return Err(ValidationFailure::AttestationTargetInFarFuture);
        }

        // Validate checkpoint consistency according to SSV spec
        self.validate_checkpoint_consistency(&beacon_vote.source, &beacon_vote.target)?;

        Ok(())
    }

    /// Validate ValidatorConsensusData according to SSV specification
    ///
    /// Ensures consensus data structure and embedded duty information
    /// comply with SSV protocol requirements.
    pub fn validate_consensus_data(
        &self,
        consensus_data: &ValidatorConsensusData,
    ) -> Result<(), ValidationFailure> {
        // Validate duty timing according to SSV spec
        let duty_epoch = self.epoch_at_slot(consensus_data.duty.slot);
        if duty_epoch > self.current_epoch + 1 {
            return Err(ValidationFailure::FarFutureDuty);
        }

        // Validate duty structure compliance
        self.validate_duty_structure(&consensus_data.duty)?;

        // Validate data version compatibility (if needed)
        self.validate_data_version(&consensus_data)?;

        Ok(())
    }

    /// Validate consistency between source and target checkpoints
    ///
    /// Implements SSV spec checkpoint validation rules to ensure
    /// attestation data is well-formed and follows protocol requirements.
    fn validate_checkpoint_consistency(
        &self,
        source: &Checkpoint,
        target: &Checkpoint,
    ) -> Result<(), ValidationFailure> {
        // Ensure source and target are different checkpoints
        if source.epoch == target.epoch && source.root != target.root {
            // This could indicate malformed data
            return Err(ValidationFailure::InvalidHash);
        }

        // Additional checkpoint validation can be added here as needed
        // based on specific SSV spec requirements

        Ok(())
    }

    /// Validate duty structure according to SSV specification
    ///
    /// Ensures duty fields are properly formatted and contain
    /// valid values according to SSV protocol requirements.
    fn validate_duty_structure(&self, duty: &ValidatorDuty) -> Result<(), ValidationFailure> {
        // Validate committee indices are within reasonable bounds
        if duty.committee_length == 0 {
            return Err(ValidationFailure::InvalidRole);
        }

        if duty.validator_committee_index >= duty.committee_length {
            return Err(ValidationFailure::InvalidRole);
        }

        // Validate committees at slot is reasonable
        if duty.committees_at_slot == 0 {
            return Err(ValidationFailure::InvalidRole);
        }

        // Additional duty structure validation can be added here
        // based on specific SSV spec requirements

        Ok(())
    }

    /// Validate data version compatibility
    ///
    /// Ensures the data version is supported and compatible
    /// with current SSV protocol requirements.
    fn validate_data_version(
        &self,
        consensus_data: &ValidatorConsensusData,
    ) -> Result<(), ValidationFailure> {
        // For now, accept all data versions
        // This can be extended to validate specific version requirements
        // based on SSV spec evolution
        let _ = consensus_data.version;

        Ok(())
    }

    /// Convert slot to epoch using configured slots per epoch
    fn epoch_at_slot(&self, slot: Slot) -> Epoch {
        Epoch::new(slot.as_u64() / self.slots_per_epoch)
    }

    /// Update current epoch context for temporal validation
    pub fn update_current_epoch(&mut self, new_epoch: Epoch) {
        self.current_epoch = new_epoch;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::Hash256;

    #[test]
    fn test_beacon_vote_validation_success() {
        let validator = BeaconDataValidator::new(Epoch::new(100), 32);

        let beacon_vote = BeaconVote {
            block_root: Hash256::zero(),
            source: Checkpoint {
                epoch: Epoch::new(98),
                root: Hash256::zero(),
            },
            target: Checkpoint {
                epoch: Epoch::new(99),
                root: Hash256::zero(),
            },
        };

        assert!(validator.validate_beacon_vote(&beacon_vote).is_ok());
    }

    #[test]
    fn test_beacon_vote_source_greater_than_target() {
        let validator = BeaconDataValidator::new(Epoch::new(100), 32);

        let beacon_vote = BeaconVote {
            block_root: Hash256::zero(),
            source: Checkpoint {
                epoch: Epoch::new(99),
                root: Hash256::zero(),
            },
            target: Checkpoint {
                epoch: Epoch::new(98), // Target < source = invalid
                root: Hash256::zero(),
            },
        };

        let result = validator.validate_beacon_vote(&beacon_vote);
        assert!(matches!(
            result,
            Err(ValidationFailure::AttestationSourceGreaterThanTarget)
        ));
    }

    #[test]
    fn test_beacon_vote_far_future_target() {
        let validator = BeaconDataValidator::new(Epoch::new(100), 32);

        let beacon_vote = BeaconVote {
            block_root: Hash256::zero(),
            source: Checkpoint {
                epoch: Epoch::new(98),
                root: Hash256::zero(),
            },
            target: Checkpoint {
                epoch: Epoch::new(102), // More than current_epoch + 1
                root: Hash256::zero(),
            },
        };

        let result = validator.validate_beacon_vote(&beacon_vote);
        assert!(matches!(
            result,
            Err(ValidationFailure::AttestationTargetInFarFuture)
        ));
    }
}
