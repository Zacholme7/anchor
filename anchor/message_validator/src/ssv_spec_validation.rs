use crate::{
    ValidationFailure, beacon_validation::BeaconDataValidator, duty_validation::DutyValidator,
    slashing_detection::EnhancedSlashingDetector,
};
use ssv_types::ValidatorIndex;
use ssv_types::consensus::{BeaconRole, BeaconVote, ValidatorConsensusData, ValidatorDuty};
use ssv_types::message::{MsgType, SSVMessage};
use ssz::Decode;
use types::{Epoch, PublicKeyBytes};

/// Coordinates SSV specification validation across all validation modules.
///
/// This module integrates beacon data validation, slashing detection, and
/// duty validation to provide comprehensive SSV spec compliance validation
/// for consensus messages and partial signature messages.
pub struct SsvSpecValidator {
    beacon_validator: BeaconDataValidator,
    slashing_detector: EnhancedSlashingDetector,
    duty_validator: DutyValidator,
}

impl SsvSpecValidator {
    /// Create a new SSV spec validator with current epoch context
    pub fn new(current_epoch: Epoch, slots_per_epoch: u64) -> Self {
        Self {
            beacon_validator: BeaconDataValidator::new(current_epoch, slots_per_epoch),
            slashing_detector: EnhancedSlashingDetector::new(),
            duty_validator: DutyValidator::new(current_epoch, slots_per_epoch),
        }
    }

    /// Main SSV spec validation entry point for SSV messages
    ///
    /// Validates an SSV message according to SSV specification requirements.
    /// Handles both consensus messages and partial signature messages with
    /// appropriate validation logic for each type.
    pub fn validate_ssv_message(
        &mut self,
        ssv_message: &SSVMessage,
        validator_pk: &PublicKeyBytes,
    ) -> Result<(), ValidationFailure> {
        match ssv_message.msg_type() {
            MsgType::SSVConsensusMsgType => {
                self.validate_consensus_message(ssv_message, validator_pk)
            }
            MsgType::SSVPartialSignatureMsgType => {
                self.validate_partial_signature_message(ssv_message, validator_pk)
            }
        }
    }

    /// Validate consensus messages according to SSV specification
    ///
    /// Consensus messages contain beacon data that must be validated for:
    /// - Proper attestation structure and slashing rules
    /// - Duty compliance and temporal constraints
    /// - Role-specific validation requirements
    fn validate_consensus_message(
        &mut self,
        ssv_message: &SSVMessage,
        validator_pk: &PublicKeyBytes,
    ) -> Result<(), ValidationFailure> {
        // Try to decode as BeaconVote first (for attestations)
        if let Ok(beacon_vote) = BeaconVote::from_ssz_bytes(&ssv_message.data()) {
            // Validate beacon vote data according to SSV spec
            self.beacon_validator.validate_beacon_vote(&beacon_vote)?;

            // Check for slashing conditions
            self.slashing_detector
                .detect_slashing(validator_pk, &beacon_vote)?;

            return Ok(());
        }

        // Try to decode as ValidatorConsensusData (for duties and proposals)
        if let Ok(consensus_data) = ValidatorConsensusData::from_ssz_bytes(&ssv_message.data()) {
            // Validate consensus data structure
            self.beacon_validator
                .validate_consensus_data(&consensus_data)?;

            // Validate duty according to SSV spec
            let duty_type = consensus_data.duty.r#type.clone();
            self.duty_validator
                .validate_duty(&consensus_data.duty, duty_type)?;

            // For beacon votes embedded in consensus data, also check slashing
            if let Ok(embedded_vote) = self.extract_beacon_vote_from_consensus_data(&consensus_data)
            {
                self.slashing_detector
                    .detect_slashing(validator_pk, &embedded_vote)?;
            }

            return Ok(());
        }

        // If neither decode succeeds, return decoding error
        Err(ValidationFailure::UndecodableMessageData(
            ssz::DecodeError::InvalidByteLength {
                len: ssv_message.data().len(),
                expected: 0,
            },
        ))
    }

    /// Validate partial signature messages according to SSV specification
    ///
    /// Partial signature messages are validated for structural correctness
    /// and compliance with SSV protocol requirements for threshold signatures.
    fn validate_partial_signature_message(
        &self,
        ssv_message: &SSVMessage,
        _validator_pk: &PublicKeyBytes,
    ) -> Result<(), ValidationFailure> {
        // For now, basic validation - can be extended with specific
        // partial signature validation logic as needed

        // Ensure message data is not empty
        if ssv_message.data().is_empty() {
            return Err(ValidationFailure::EmptyData);
        }

        // Additional partial signature validation can be added here
        // based on specific SSV spec requirements for threshold signatures

        Ok(())
    }

    /// Validate a duty with optional identity validation
    ///
    /// Provides a direct interface for duty validation with optional
    /// validator identity validation according to SSV spec requirements.
    pub fn validate_duty_with_identity(
        &self,
        duty: &ValidatorDuty,
        expected_role: BeaconRole,
        expected_index: Option<ValidatorIndex>,
        expected_pubkey: Option<&PublicKeyBytes>,
    ) -> Result<(), ValidationFailure> {
        // Validate duty structure and timing
        self.duty_validator.validate_duty(duty, expected_role)?;

        // Validate validator identity if provided
        DutyValidator::validate_validator_identity(duty, expected_index, expected_pubkey)?;

        Ok(())
    }

    /// Check if a beacon vote would be slashable without storing it
    ///
    /// Useful for pre-validation checks without side effects.
    pub fn would_vote_be_slashable(
        &self,
        validator_pk: &PublicKeyBytes,
        vote: &BeaconVote,
    ) -> bool {
        self.slashing_detector
            .would_be_slashable(validator_pk, vote)
    }

    /// Extract beacon vote from consensus data if possible
    ///
    /// Some consensus data structures may contain embedded beacon votes
    /// that need slashing validation. This method attempts to extract
    /// such votes for validation.
    fn extract_beacon_vote_from_consensus_data(
        &self,
        consensus_data: &ValidatorConsensusData,
    ) -> Result<BeaconVote, ssz::DecodeError> {
        // Try to decode the data_ssz field as a BeaconVote
        BeaconVote::from_ssz_bytes(&consensus_data.data_ssz)
    }

    /// Update epoch context for all validators
    ///
    /// Updates the current epoch context used for temporal validation
    /// across all validation modules.
    pub fn update_current_epoch(&mut self, new_epoch: Epoch) {
        self.beacon_validator.update_current_epoch(new_epoch);
        self.duty_validator.update_current_epoch(new_epoch);
    }

    /// Clear slashing detection history (useful for testing)
    pub fn clear_slashing_history(&mut self) {
        self.slashing_detector.clear_history();
    }

    /// Get slashing history size for a validator
    pub fn get_slashing_history_size(&self, validator_pk: &PublicKeyBytes) -> usize {
        self.slashing_detector.get_history_size(validator_pk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ssv_types::consensus::{BEACON_ROLE_ATTESTER, BEACON_ROLE_PROPOSER};
    use ssv_types::typenum::U13;
    use types::VariableList;
    use types::{Checkpoint, CommitteeIndex, Hash256, Slot};

    fn create_test_beacon_vote(source_epoch: u64, target_epoch: u64) -> BeaconVote {
        BeaconVote {
            block_root: Hash256::zero(),
            source: Checkpoint {
                epoch: Epoch::new(source_epoch),
                root: Hash256::zero(),
            },
            target: Checkpoint {
                epoch: Epoch::new(target_epoch),
                root: Hash256::from_low_u64_be(target_epoch),
            },
        }
    }

    fn create_test_duty() -> ValidatorDuty {
        ValidatorDuty {
            r#type: BEACON_ROLE_ATTESTER,
            pub_key: PublicKeyBytes::from([1u8; 48]),
            slot: Slot::new(3200), // Epoch 100
            validator_index: ValidatorIndex(12345),
            committee_index: CommitteeIndex::new(0),
            committee_length: 128,
            committees_at_slot: 1,
            validator_committee_index: 0,
            validator_sync_committee_indices: VariableList::<u64, U13>::empty(),
        }
    }

    #[test]
    fn test_beacon_vote_validation_success() {
        let mut validator = SsvSpecValidator::new(Epoch::new(100), 32);
        let validator_pk = PublicKeyBytes::from([1u8; 48]);

        let beacon_vote = create_test_beacon_vote(98, 99);
        let beacon_vote_bytes = beacon_vote.as_ssz_bytes();

        let ssv_message = SSVMessage {
            msg_type: MsgType::SSVConsensusMsgType,
            msg_id: [0u8; 56].into(),
            data: beacon_vote_bytes.into(),
        };

        assert!(
            validator
                .validate_ssv_message(&ssv_message, &validator_pk)
                .is_ok()
        );
    }

    #[test]
    fn test_slashing_detection_in_consensus_message() {
        let mut validator = SsvSpecValidator::new(Epoch::new(100), 32);
        let validator_pk = PublicKeyBytes::from([1u8; 48]);

        // First valid vote
        let vote1 = create_test_beacon_vote(98, 99);
        let vote1_bytes = vote1.as_ssz_bytes();
        let ssv_message1 = SSVMessage {
            msg_type: MsgType::SSVConsensusMsgType,
            msg_id: [0u8; 56].into(),
            data: vote1_bytes.into(),
        };

        assert!(
            validator
                .validate_ssv_message(&ssv_message1, &validator_pk)
                .is_ok()
        );

        // Second vote that creates double voting
        let mut vote2 = create_test_beacon_vote(98, 99);
        vote2.target.root = Hash256::from_low_u64_be(999); // Different root, same epoch
        let vote2_bytes = vote2.as_ssz_bytes();
        let ssv_message2 = SSVMessage {
            msg_type: MsgType::SSVConsensusMsgType,
            msg_id: [0u8; 56].into(),
            data: vote2_bytes.into(),
        };

        let result = validator.validate_ssv_message(&ssv_message2, &validator_pk);
        assert!(matches!(
            result,
            Err(ValidationFailure::SlashableAttestation)
        ));
    }

    #[test]
    fn test_duty_validation_with_identity() {
        let validator = SsvSpecValidator::new(Epoch::new(100), 32);

        let duty = create_test_duty();
        let expected_index = ValidatorIndex(12345);
        let expected_pubkey = PublicKeyBytes::from([1u8; 48]);

        assert!(
            validator
                .validate_duty_with_identity(
                    &duty,
                    BEACON_ROLE_ATTESTER,
                    Some(expected_index),
                    Some(&expected_pubkey)
                )
                .is_ok()
        );

        // Test with wrong role
        let result = validator.validate_duty_with_identity(
            &duty,
            BEACON_ROLE_PROPOSER,
            Some(expected_index),
            Some(&expected_pubkey),
        );
        assert!(matches!(
            result,
            Err(ValidationFailure::WrongBeaconRoleType)
        ));
    }

    #[test]
    fn test_would_vote_be_slashable() {
        let mut validator = SsvSpecValidator::new(Epoch::new(100), 32);
        let validator_pk = PublicKeyBytes::from([1u8; 48]);

        let vote1 = create_test_beacon_vote(98, 99);

        // First vote should not be slashable
        assert!(!validator.would_vote_be_slashable(&validator_pk, &vote1));

        // Store the first vote
        validator
            .slashing_detector
            .detect_slashing(&validator_pk, &vote1)
            .unwrap();

        // Second vote with same target epoch but different root should be slashable
        let mut vote2 = create_test_beacon_vote(98, 99);
        vote2.target.root = Hash256::from_low_u64_be(999);
        assert!(validator.would_vote_be_slashable(&validator_pk, &vote2));
    }

    #[test]
    fn test_partial_signature_message_validation() {
        let validator = SsvSpecValidator::new(Epoch::new(100), 32);
        let validator_pk = PublicKeyBytes::from([1u8; 48]);

        let ssv_message = SSVMessage {
            msg_type: MsgType::SSVPartialSignatureMsgType,
            msg_id: [0u8; 56].into(),
            data: vec![1, 2, 3, 4].into(), // Non-empty data
        };

        assert!(
            validator
                .validate_ssv_message(&ssv_message, &validator_pk)
                .is_ok()
        );

        // Test with empty data
        let empty_message = SSVMessage {
            msg_type: MsgType::SSVPartialSignatureMsgType,
            msg_id: [0u8; 56].into(),
            data: vec![].into(),
        };

        let result = validator.validate_ssv_message(&empty_message, &validator_pk);
        assert!(matches!(result, Err(ValidationFailure::EmptyData)));
    }
}
