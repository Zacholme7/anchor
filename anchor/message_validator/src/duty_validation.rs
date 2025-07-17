use crate::ValidationFailure;
use ssv_types::ValidatorIndex;
use ssv_types::consensus::{
    BEACON_ROLE_AGGREGATOR, BEACON_ROLE_ATTESTER, BEACON_ROLE_PROPOSER,
    BEACON_ROLE_SYNC_COMMITTEE_CONTRIBUTION, BeaconRole, ValidatorDuty,
};
use types::{Epoch, PublicKeyBytes, Slot};

/// Validates validator duties according to SSV specification requirements.
///
/// This module ensures that duties are properly formed, temporally valid,
/// and comply with SSV protocol requirements for role assignments and
/// committee memberships.
pub struct DutyValidator {
    current_epoch: Epoch,
    slots_per_epoch: u64,
}

impl DutyValidator {
    /// Create a new duty validator with current epoch context
    pub fn new(current_epoch: Epoch, slots_per_epoch: u64) -> Self {
        Self {
            current_epoch,
            slots_per_epoch,
        }
    }

    /// Validate duty according to SSV specification requirements
    ///
    /// Performs comprehensive duty validation including:
    /// - Role validation against expected beacon role
    /// - Temporal validation for duty timing
    /// - Committee assignment validation
    /// - Structural validation of duty fields
    pub fn validate_duty(
        &self,
        duty: &ValidatorDuty,
        expected_role: BeaconRole,
    ) -> Result<(), ValidationFailure> {
        // SSV Spec: Role validation - ensure duty role matches expected role
        if duty.r#type != expected_role {
            return Err(ValidationFailure::WrongBeaconRoleType);
        }

        // SSV Spec: Temporal validation - duty cannot be too far in future
        let duty_epoch = self.epoch_at_slot(duty.slot);
        if duty_epoch > self.current_epoch + 1 {
            return Err(ValidationFailure::FarFutureDuty);
        }

        // SSV Spec: Committee assignment validation
        self.validate_committee_assignment(duty)?;

        // SSV Spec: Role-specific validation
        self.validate_role_specific_requirements(duty)?;

        // SSV Spec: Structural validation
        self.validate_duty_structure(duty)?;

        Ok(())
    }

    /// Validate duty against expected validator identity
    ///
    /// Ensures the duty is assigned to the expected validator by checking
    /// validator index and public key according to SSV spec requirements.
    pub fn validate_validator_identity(
        duty: &ValidatorDuty,
        expected_index: Option<ValidatorIndex>,
        expected_pubkey: Option<&PublicKeyBytes>,
    ) -> Result<(), ValidationFailure> {
        // SSV Spec: Validator index validation
        if let Some(expected_idx) = expected_index {
            if duty.validator_index != expected_idx {
                return Err(ValidationFailure::WrongValidatorIndex);
            }
        }

        // SSV Spec: Public key validation
        if let Some(expected_pk) = expected_pubkey {
            if duty.pub_key != *expected_pk {
                return Err(ValidationFailure::WrongValidatorPk);
            }
        }

        Ok(())
    }

    /// Validate committee assignment according to SSV specification
    ///
    /// Ensures committee-related fields are consistent and within valid ranges
    /// according to beacon chain and SSV protocol requirements.
    fn validate_committee_assignment(&self, duty: &ValidatorDuty) -> Result<(), ValidationFailure> {
        // Committee length must be positive
        if duty.committee_length == 0 {
            return Err(ValidationFailure::InvalidRole);
        }

        // Validator committee index must be within committee bounds
        if duty.validator_committee_index >= duty.committee_length {
            return Err(ValidationFailure::InvalidRole);
        }

        // Committees at slot must be positive
        if duty.committees_at_slot == 0 {
            return Err(ValidationFailure::InvalidRole);
        }

        // Committee index must be within committees at slot bounds
        if u64::from(duty.committee_index) >= duty.committees_at_slot {
            return Err(ValidationFailure::InvalidRole);
        }

        Ok(())
    }

    /// Validate role-specific requirements according to SSV specification
    ///
    /// Different beacon roles have different requirements and constraints
    /// that must be validated according to SSV protocol rules.
    fn validate_role_specific_requirements(
        &self,
        duty: &ValidatorDuty,
    ) -> Result<(), ValidationFailure> {
        match duty.r#type {
            BEACON_ROLE_ATTESTER => {
                // Attestation duties require valid committee assignment
                self.validate_committee_assignment(duty)?;
            }
            BEACON_ROLE_AGGREGATOR => {
                // Aggregator duties require valid committee assignment
                self.validate_committee_assignment(duty)?;
                // Additional aggregator-specific validation could go here
            }
            BEACON_ROLE_PROPOSER => {
                // Proposer duties have different requirements
                // Committee fields may not be as relevant for proposers
                // Additional proposer-specific validation could go here
            }
            BEACON_ROLE_SYNC_COMMITTEE_CONTRIBUTION => {
                // Sync committee duties require sync committee indices
                if duty.validator_sync_committee_indices.is_empty() {
                    return Err(ValidationFailure::InvalidRole);
                }
                // Additional sync committee-specific validation could go here
            }
            _ => {
                // Unknown or unsupported role
                return Err(ValidationFailure::InvalidRole);
            }
        }

        Ok(())
    }

    /// Validate duty structure fields according to SSV specification
    ///
    /// Ensures all duty fields contain reasonable values and are
    /// internally consistent according to SSV protocol requirements.
    fn validate_duty_structure(&self, duty: &ValidatorDuty) -> Result<(), ValidationFailure> {
        // Slot must be valid (not in far past or future)
        let current_slot = self.current_epoch.as_u64() * self.slots_per_epoch;
        let duty_slot = duty.slot.as_u64();

        // Don't allow duties too far in the past (configurable threshold)
        let max_past_slots = self.slots_per_epoch * 2; // 2 epochs in past
        if duty_slot + max_past_slots < current_slot {
            return Err(ValidationFailure::LateSlotMessage {
                got: duty.slot.to_string(),
            });
        }

        // Validator index must be reasonable (not extremely large)
        let max_validator_index = 2_000_000; // Reasonable upper bound
        if duty.validator_index.0 > max_validator_index {
            return Err(ValidationFailure::WrongValidatorIndex);
        }

        // Public key must not be empty (all zeros)
        let empty_pubkey = PublicKeyBytes::empty();
        if duty.pub_key == empty_pubkey {
            return Err(ValidationFailure::WrongValidatorPk);
        }

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

    /// Check if a duty is for a future epoch
    pub fn is_future_duty(&self, duty: &ValidatorDuty) -> bool {
        let duty_epoch = self.epoch_at_slot(duty.slot);
        duty_epoch > self.current_epoch
    }

    /// Check if a duty is for the current epoch
    pub fn is_current_duty(&self, duty: &ValidatorDuty) -> bool {
        let duty_epoch = self.epoch_at_slot(duty.slot);
        duty_epoch == self.current_epoch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ssv_types::typenum::U13;
    use types::CommitteeIndex;
    use types::VariableList;

    fn create_test_duty(role: BeaconRole, slot: u64, validator_index: usize) -> ValidatorDuty {
        ValidatorDuty {
            r#type: role,
            pub_key: PublicKeyBytes::from([1u8; 48]), // Non-empty pubkey
            slot: Slot::new(slot),
            validator_index: ValidatorIndex(validator_index),
            committee_index: CommitteeIndex::new(0),
            committee_length: 128,
            committees_at_slot: 1,
            validator_committee_index: 0,
            validator_sync_committee_indices: VariableList::<u64, U13>::empty(),
        }
    }

    #[test]
    fn test_valid_duty_validation() {
        let validator = DutyValidator::new(Epoch::new(100), 32);

        let duty = create_test_duty(BEACON_ROLE_ATTESTER, 3200, 12345); // Epoch 100
        assert!(validator.validate_duty(&duty, BEACON_ROLE_ATTESTER).is_ok());
    }

    #[test]
    fn test_wrong_beacon_role_type() {
        let validator = DutyValidator::new(Epoch::new(100), 32);

        let duty = create_test_duty(BEACON_ROLE_ATTESTER, 3200, 12345);
        let result = validator.validate_duty(&duty, BEACON_ROLE_PROPOSER);
        assert!(matches!(
            result,
            Err(ValidationFailure::WrongBeaconRoleType)
        ));
    }

    #[test]
    fn test_far_future_duty() {
        let validator = DutyValidator::new(Epoch::new(100), 32);

        let duty = create_test_duty(BEACON_ROLE_ATTESTER, 3264, 12345); // Epoch 102 (too far)
        let result = validator.validate_duty(&duty, BEACON_ROLE_ATTESTER);
        assert!(matches!(result, Err(ValidationFailure::FarFutureDuty)));
    }

    #[test]
    fn test_invalid_committee_assignment() {
        let validator = DutyValidator::new(Epoch::new(100), 32);

        let mut duty = create_test_duty(BEACON_ROLE_ATTESTER, 3200, 12345);
        duty.validator_committee_index = 128; // >= committee_length (128)

        let result = validator.validate_duty(&duty, BEACON_ROLE_ATTESTER);
        assert!(matches!(result, Err(ValidationFailure::InvalidRole)));
    }

    #[test]
    fn test_validator_identity_validation() {
        let expected_index = ValidatorIndex(12345);
        let expected_pubkey = PublicKeyBytes::from([1u8; 48]);

        let duty = create_test_duty(BEACON_ROLE_ATTESTER, 3200, 12345);

        // Should pass with correct identity
        assert!(
            DutyValidator::validate_validator_identity(
                &duty,
                Some(expected_index),
                Some(&expected_pubkey)
            )
            .is_ok()
        );

        // Should fail with wrong index
        let wrong_index = ValidatorIndex(54321);
        let result = DutyValidator::validate_validator_identity(
            &duty,
            Some(wrong_index),
            Some(&expected_pubkey),
        );
        assert!(matches!(
            result,
            Err(ValidationFailure::WrongValidatorIndex)
        ));

        // Should fail with wrong pubkey
        let wrong_pubkey = PublicKeyBytes::from([2u8; 48]);
        let result = DutyValidator::validate_validator_identity(
            &duty,
            Some(expected_index),
            Some(&wrong_pubkey),
        );
        assert!(matches!(result, Err(ValidationFailure::WrongValidatorPk)));
    }

    #[test]
    fn test_empty_pubkey_validation() {
        let validator = DutyValidator::new(Epoch::new(100), 32);

        let mut duty = create_test_duty(BEACON_ROLE_ATTESTER, 3200, 12345);
        duty.pub_key = PublicKeyBytes::empty(); // Invalid empty pubkey

        let result = validator.validate_duty(&duty, BEACON_ROLE_ATTESTER);
        assert!(matches!(result, Err(ValidationFailure::WrongValidatorPk)));
    }

    #[test]
    fn test_sync_committee_role_validation() {
        let validator = DutyValidator::new(Epoch::new(100), 32);

        let duty = create_test_duty(BEACON_ROLE_SYNC_COMMITTEE_CONTRIBUTION, 3200, 12345);

        // Should fail because sync committee indices are empty
        let result = validator.validate_duty(&duty, BEACON_ROLE_SYNC_COMMITTEE_CONTRIBUTION);
        assert!(matches!(result, Err(ValidationFailure::InvalidRole)));
    }
}
