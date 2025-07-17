# Extend Message Validator with SSV Spec Validation Implementation Plan

## Executive Summary

Extend the production `message_validator` crate with SSV specification-compliant validation logic to ensure our validator implementation correctly follows the SSV protocol. This will make both our production code more robust and enable the SSV spec tests to validate real production behavior.

**Current Gap**: Our `message_validator` has excellent infrastructure (message structure, signatures, timing) but lacks SSV-specific validation logic (beacon data validation, enhanced slashing detection, spec-compliant duty validation).

**Solution**: Add SSV spec validation modules to `message_validator` that implement the missing validation logic according to the official SSV specification.

### Architecture Enhancement
```
BEFORE:
message_validator::validate() → Basic validation → ValidationResult

AFTER:  
message_validator::validate() → Basic validation → SSV Spec validation → ValidationResult
                                                      ↓
                                              • Beacon data validation
                                              • Enhanced slashing detection  
                                              • Spec-compliant duty validation
                                              • Validator identity validation
```

### Expected Outcomes
- Production `message_validator` becomes fully SSV spec-compliant
- All 12 SSV validation spec tests pass using real production validation
- Enhanced slashing protection and safety guarantees in production
- Foundation for implementing remaining 252 SSV spec tests

## Goals & Objectives

### Primary Goals
- **Extend production validation** with SSV spec-compliant validation logic
- **Achieve 12/12 SSV validation spec tests passing** using enhanced production validator
- **Maintain backward compatibility** with existing message_validator usage

### Secondary Objectives
- **Improve production safety** with enhanced slashing detection and beacon data validation
- **Establish foundation** for complete SSV spec test implementation
- **Document SSV spec compliance** in validation logic

## Solution Overview

### Approach
Add new validation modules to the existing `message_validator` crate that implement SSV specification requirements. Integrate these modules into the main validation flow while maintaining existing functionality.

### Key Components
1. **Beacon Data Validation**: Validate attestation data, proposal values, and consensus data structures
2. **Enhanced Slashing Detection**: Implement SSV-specific slashing rules and cross-validator checks
3. **Spec-Compliant Duty Validation**: Validate duties according to SSV specification requirements
4. **Validator Identity Validation**: Ensure validator indices and public keys match expected values
5. **Integration Layer**: Seamlessly integrate new validation into existing flow

### Expected Outcomes
- Enhanced production validation that prevents SSV spec violations
- All SSV validation spec tests pass using production validation logic
- Backward compatibility with existing message_validator usage
- Foundation for implementing additional SSV spec test categories

## Implementation Tasks

### CRITICAL IMPLEMENTATION RULES
1. **MAINTAIN PRODUCTION STABILITY**: All changes must be backward compatible and not break existing functionality
2. **FOLLOW SSV SPECIFICATION**: Implement validation logic exactly according to SSV spec requirements
3. **COMPREHENSIVE ERROR MAPPING**: Ensure all new ValidationFailure types map to expected test outputs
4. **PERFORMANCE CONSCIOUS**: New validation should not significantly impact message processing performance

### Visual Dependency Tree
```
anchor/message_validator/
├── src/
│   ├── lib.rs (Task #3: Integrate new validation modules)
│   ├── beacon_validation.rs (Task #0: Beacon data validation)
│   ├── slashing_detection.rs (Task #1: Enhanced slashing detection)
│   ├── duty_validation.rs (Task #1: Spec-compliant duty validation)
│   ├── validator_identity.rs (Task #1: Identity validation)
│   └── ssv_spec_validation.rs (Task #2: Integration module)
└── Cargo.toml (Task #0: Add dependencies if needed)

anchor/spec_tests/
└── src/utils/real_validator.rs (Task #4: Update error mapping for new ValidationFailure types)
```

### Execution Plan

#### Group A: Core Validation Modules (Execute in parallel)
- [ ] **Task #0**: Implement beacon data validation module
  - **File**: `anchor/message_validator/src/beacon_validation.rs`
  - **Imports**:
    ```rust
    use crate::ValidationFailure;
    use ssv_types::consensus::{BeaconVote, ValidatorConsensusData, BeaconRole};
    use types::{Epoch, Slot, Checkpoint};
    ```
  - **Implements**:
    ```rust
    pub struct BeaconDataValidator {
        current_epoch: Epoch,
        slots_per_epoch: u64,
    }
    
    impl BeaconDataValidator {
        pub fn new(current_epoch: Epoch, slots_per_epoch: u64) -> Self {
            Self { current_epoch, slots_per_epoch }
        }
        
        /// Validate BeaconVote (attestation) data according to SSV spec
        pub fn validate_beacon_vote(&self, beacon_vote: &BeaconVote) -> Result<(), ValidationFailure> {
            // SSV Spec requirement: source epoch < target epoch
            if beacon_vote.source.epoch >= beacon_vote.target.epoch {
                return Err(ValidationFailure::AttestationSourceGreaterThanTarget);
            }
            
            // SSV Spec requirement: target epoch not too far in future
            if beacon_vote.target.epoch > self.current_epoch + 1 {
                return Err(ValidationFailure::AttestationTargetInFarFuture);
            }
            
            // SSV Spec requirement: validate checkpoint consistency
            self.validate_checkpoint_consistency(&beacon_vote.source, &beacon_vote.target)?;
            
            Ok(())
        }
        
        /// Validate ValidatorConsensusData according to SSV spec
        pub fn validate_consensus_data(&self, consensus_data: &ValidatorConsensusData) -> Result<(), ValidationFailure> {
            // Validate duty timing according to SSV spec
            let duty_epoch = self.epoch_at_slot(consensus_data.duty.slot);
            if duty_epoch > self.current_epoch + 1 {
                return Err(ValidationFailure::FarFutureDuty);
            }
            
            // Validate duty structure
            self.validate_duty_structure(&consensus_data.duty)?;
            
            Ok(())
        }
        
        fn validate_checkpoint_consistency(&self, source: &Checkpoint, target: &Checkpoint) -> Result<(), ValidationFailure> {
            // Implement SSV spec checkpoint validation rules
            Ok(())
        }
        
        fn validate_duty_structure(&self, duty: &ValidatorDuty) -> Result<(), ValidationFailure> {
            // Implement SSV spec duty structure validation
            Ok(())
        }
        
        fn epoch_at_slot(&self, slot: Slot) -> Epoch {
            Epoch::new(slot.as_u64() / self.slots_per_epoch)
        }
    }
    ```
  - **Exports**: `BeaconDataValidator` struct and validation methods
  - **Context**: Core beacon chain data validation according to SSV specification

- [ ] **Task #1**: Implement enhanced slashing detection and duty validation modules
  - **Files**: 
    - `anchor/message_validator/src/slashing_detection.rs`
    - `anchor/message_validator/src/duty_validation.rs` 
    - `anchor/message_validator/src/validator_identity.rs`
  - **Slashing Detection Module**:
    ```rust
    // slashing_detection.rs
    use crate::ValidationFailure;
    use ssv_types::consensus::BeaconVote;
    use std::collections::HashMap;
    use types::{Epoch, PublicKeyBytes};
    
    pub struct EnhancedSlashingDetector {
        attestation_history: HashMap<PublicKeyBytes, Vec<BeaconVote>>,
    }
    
    impl EnhancedSlashingDetector {
        pub fn new() -> Self {
            Self {
                attestation_history: HashMap::new(),
            }
        }
        
        /// Detect slashing conditions according to SSV spec
        pub fn detect_slashing(&mut self, validator_pk: &PublicKeyBytes, new_vote: &BeaconVote) -> Result<(), ValidationFailure> {
            let history = self.attestation_history.entry(*validator_pk).or_insert_with(Vec::new);
            
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
            
            // Store new vote for future slashing detection
            history.push(new_vote.clone());
            
            Ok(())
        }
        
        fn is_double_vote(&self, vote1: &BeaconVote, vote2: &BeaconVote) -> bool {
            // Same target epoch, different target roots = double vote
            vote1.target.epoch == vote2.target.epoch && vote1.target.root != vote2.target.root
        }
        
        fn is_surround_vote(&self, vote1: &BeaconVote, vote2: &BeaconVote) -> bool {
            // SSV spec surround voting rules
            (vote1.source.epoch < vote2.source.epoch && vote1.target.epoch > vote2.target.epoch) ||
            (vote2.source.epoch < vote1.source.epoch && vote2.target.epoch > vote1.target.epoch)
        }
    }
    ```
  - **Duty Validation Module**:
    ```rust
    // duty_validation.rs
    use crate::ValidationFailure;
    use ssv_types::consensus::{ValidatorDuty, BeaconRole};
    use ssv_types::ValidatorIndex;
    use types::{PublicKeyBytes, Slot, Epoch};
    
    pub struct DutyValidator {
        current_epoch: Epoch,
    }
    
    impl DutyValidator {
        pub fn new(current_epoch: Epoch) -> Self {
            Self { current_epoch }
        }
        
        /// Validate duty according to SSV spec requirements
        pub fn validate_duty(&self, duty: &ValidatorDuty, expected_role: BeaconRole) -> Result<(), ValidationFailure> {
            // SSV Spec: Role validation
            if duty.r#type != expected_role {
                return Err(ValidationFailure::WrongBeaconRoleType);
            }
            
            // SSV Spec: Temporal validation
            let duty_epoch = self.epoch_at_slot(duty.slot);
            if duty_epoch > self.current_epoch + 1 {
                return Err(ValidationFailure::FarFutureDuty);
            }
            
            // SSV Spec: Committee validation
            self.validate_committee_assignment(duty)?;
            
            Ok(())
        }
        
        fn validate_committee_assignment(&self, duty: &ValidatorDuty) -> Result<(), ValidationFailure> {
            // Implement SSV spec committee assignment validation
            Ok(())
        }
        
        fn epoch_at_slot(&self, slot: Slot) -> Epoch {
            Epoch::new(slot.as_u64() / 32) // Assuming 32 slots per epoch
        }
    }
    ```
  - **Validator Identity Module**:
    ```rust
    // validator_identity.rs
    use crate::ValidationFailure;
    use ssv_types::{ValidatorIndex, consensus::ValidatorDuty};
    use types::PublicKeyBytes;
    
    pub struct ValidatorIdentityValidator;
    
    impl ValidatorIdentityValidator {
        /// Validate validator identity according to SSV spec
        pub fn validate_identity(
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
    }
    ```
  - **Context**: Enhanced validation modules implementing SSV spec requirements

#### Group B: Integration Layer (Execute after Group A)
- [ ] **Task #2**: Create SSV spec validation integration module
  - **File**: `anchor/message_validator/src/ssv_spec_validation.rs`
  - **Imports**:
    ```rust
    use crate::{ValidationFailure, beacon_validation::BeaconDataValidator, 
                slashing_detection::EnhancedSlashingDetector,
                duty_validation::DutyValidator,
                validator_identity::ValidatorIdentityValidator};
    use ssv_types::consensus::{BeaconVote, ValidatorConsensusData, ValidatorDuty};
    use ssv_types::message::{SSVMessage, MsgType};
    use types::{Epoch, PublicKeyBytes};
    ```
  - **Implements**:
    ```rust
    pub struct SsvSpecValidator {
        beacon_validator: BeaconDataValidator,
        slashing_detector: EnhancedSlashingDetector,
        duty_validator: DutyValidator,
    }
    
    impl SsvSpecValidator {
        pub fn new(current_epoch: Epoch, slots_per_epoch: u64) -> Self {
            Self {
                beacon_validator: BeaconDataValidator::new(current_epoch, slots_per_epoch),
                slashing_detector: EnhancedSlashingDetector::new(),
                duty_validator: DutyValidator::new(current_epoch),
            }
        }
        
        /// Main SSV spec validation entry point
        pub fn validate_ssv_message(&mut self, ssv_message: &SSVMessage, validator_pk: &PublicKeyBytes) -> Result<(), ValidationFailure> {
            match ssv_message.msg_type {
                MsgType::SSVConsensusMsgType => {
                    self.validate_consensus_message(ssv_message, validator_pk)
                },
                MsgType::SSVPartialSignatureMsgType => {
                    self.validate_partial_signature_message(ssv_message, validator_pk)
                },
            }
        }
        
        fn validate_consensus_message(&mut self, ssv_message: &SSVMessage, validator_pk: &PublicKeyBytes) -> Result<(), ValidationFailure> {
            // Try to decode as BeaconVote first (for attestations)
            if let Ok(beacon_vote) = BeaconVote::from_ssz_bytes(&ssv_message.data) {
                self.beacon_validator.validate_beacon_vote(&beacon_vote)?;
                self.slashing_detector.detect_slashing(validator_pk, &beacon_vote)?;
                return Ok(());
            }
            
            // Try to decode as ValidatorConsensusData (for duties)
            if let Ok(consensus_data) = ValidatorConsensusData::from_ssz_bytes(&ssv_message.data) {
                self.beacon_validator.validate_consensus_data(&consensus_data)?;
                self.duty_validator.validate_duty(&consensus_data.duty, consensus_data.duty.r#type)?;
                return Ok(());
            }
            
            // If neither decode succeeds, return decoding error
            Err(ValidationFailure::UndecodableMessageData(ssz::DecodeError::InvalidByteLength { len: ssv_message.data.len(), expected: 0 }))
        }
        
        fn validate_partial_signature_message(&self, ssv_message: &SSVMessage, _validator_pk: &PublicKeyBytes) -> Result<(), ValidationFailure> {
            // Implement partial signature validation according to SSV spec
            Ok(())
        }
    }
    ```
  - **Exports**: `SsvSpecValidator` for integration with main validator
  - **Context**: Coordinates all SSV spec validation modules

- [ ] **Task #3**: Integrate SSV spec validation into main message_validator flow
  - **File**: `anchor/message_validator/src/lib.rs`
  - **Add imports**:
    ```rust
    mod beacon_validation;
    mod slashing_detection;
    mod duty_validation;
    mod validator_identity;
    mod ssv_spec_validation;
    
    use ssv_spec_validation::SsvSpecValidator;
    ```
  - **Extend Validator struct**:
    ```rust
    pub struct Validator<S: SlotClock + 'static, D: DutiesProvider> {
        // ... existing fields ...
        ssv_spec_validator: std::sync::Mutex<SsvSpecValidator>,
    }
    ```
  - **Update validation flow**:
    ```rust
    impl<S: SlotClock + 'static, D: DutiesProvider> Validator<S, D> {
        pub fn new(
            // ... existing parameters ...
        ) -> Arc<Self> {
            let current_epoch = Epoch::new(100); // Get from slot clock
            let ssv_spec_validator = SsvSpecValidator::new(current_epoch, slots_per_epoch);
            
            Arc::new(Self {
                // ... existing initialization ...
                ssv_spec_validator: std::sync::Mutex::new(ssv_spec_validator),
            })
        }
        
        pub fn validate(&self, message_data: &[u8]) -> ValidationResult {
            // ... existing validation (decode SignedSSVMessage, basic validation) ...
            
            // NEW: Add SSV spec validation
            if let Some(validator_pk) = self.extract_validator_public_key(&signed_ssv_message) {
                if let Err(failure) = self.ssv_spec_validator
                    .lock()
                    .unwrap()
                    .validate_ssv_message(&signed_ssv_message.ssv_message, &validator_pk) 
                {
                    return ValidationResult::PostDecodeFailure(failure, signed_ssv_message);
                }
            }
            
            // ... continue with existing validation ...
        }
        
        fn extract_validator_public_key(&self, signed_message: &SignedSSVMessage) -> Option<PublicKeyBytes> {
            // Extract validator public key from message or context
            // Implementation depends on message structure
            None // Placeholder
        }
    }
    ```
  - **Context**: Seamlessly integrate SSV spec validation into existing production flow

#### Group C: Error Mapping and Testing (Execute after Group B)
- [ ] **Task #4**: Add new ValidationFailure types and update error mapping
  - **Files**: 
    - `anchor/message_validator/src/lib.rs` (add new ValidationFailure variants)
    - `anchor/spec_tests/src/utils/real_validator.rs` (update error mapping)
  - **Add ValidationFailure variants**:
    ```rust
    // In message_validator/src/lib.rs
    #[derive(Debug, Clone, PartialEq)]
    pub enum ValidationFailure {
        // ... existing variants ...
        
        // New SSV spec validation failures
        AttestationSourceGreaterThanTarget,
        AttestationTargetInFarFuture,
        FarFutureDuty,
        WrongBeaconRoleType,
        WrongValidatorIndex,
        WrongValidatorPk,
        SlashableAttestation,
    }
    ```
  - **Update error mapping in spec_tests**:
    ```rust
    // In spec_tests/src/utils/real_validator.rs
    pub fn map_validation_failure_to_expected_error(failure: &ValidationFailure) -> String {
        match failure {
            // ... existing mappings ...
            
            // New SSV spec error mappings
            ValidationFailure::AttestationSourceGreaterThanTarget => "attestation data source >= target".to_string(),
            ValidationFailure::AttestationTargetInFarFuture => "attestation data target epoch is into far future".to_string(),
            ValidationFailure::FarFutureDuty => "duty invalid: duty epoch is into far future".to_string(),
            ValidationFailure::WrongBeaconRoleType => "duty invalid: wrong beacon role type".to_string(),
            ValidationFailure::WrongValidatorIndex => "duty invalid: wrong validator index".to_string(),
            ValidationFailure::WrongValidatorPk => "duty invalid: wrong validator pk".to_string(),
            ValidationFailure::SlashableAttestation => "slashable attestation".to_string(),
        }
    }
    ```
  - **Test integration**: Run spec tests to verify new validation logic works
  - **Context**: Ensure new validation failures map correctly to expected test outputs

---

## Implementation Workflow

This plan file serves as the authoritative checklist for implementation. When implementing:

### Required Process
1. **Load Plan**: Read this entire plan file before starting
2. **Sync Tasks**: Create TodoWrite tasks matching the checkboxes below
3. **Execute & Update**: For each task:
   - Mark TodoWrite as `in_progress` when starting
   - Update checkbox `[ ]` to `[x]` when completing
   - Mark TodoWrite as `completed` when done
4. **Maintain Sync**: Keep this file and TodoWrite synchronized throughout

### Critical Rules
- This plan file is the source of truth for progress
- Update checkboxes in real-time as work progresses
- Never lose synchronization between plan file and TodoWrite
- Mark tasks complete only when fully implemented (no placeholders)
- Tasks in Group A can run in parallel, Groups B and C are sequential

### Success Criteria
- [ ] All new validation modules compile and integrate successfully
- [ ] All 12 SSV validation spec tests pass: `cargo test test_ssv_validation`
- [ ] No regressions in existing message_validator functionality
- [ ] New validation logic follows SSV specification exactly
- [ ] Error messages match expected spec test outputs exactly

### Progress Tracking
The checkboxes above represent the authoritative status of each task. Keep them updated as you work.