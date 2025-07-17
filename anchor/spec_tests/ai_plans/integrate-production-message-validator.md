# Integrate Production Message Validator Implementation Plan

## Executive Summary

Currently, our SSV validation tests use custom, simplified validation logic that reimplements complex validation rules. This approach has several problems:
- **Duplicate logic**: We're rewriting validation that already exists in production
- **Incomplete coverage**: Our custom validation misses edge cases handled by the real validator
- **Maintenance burden**: Changes to production validation require parallel updates to test validation
- **Test reliability**: Tests may pass with our simplified logic but fail with real validation

**Solution**: Replace our custom validation engine with the existing production `message_validator` crate that's already used throughout the Anchor application. This crate provides comprehensive validation including:
- Real slashing detection and prevention
- Production QBFT consensus validation  
- Authentic signature verification with RSA keys
- Complete duty state management and tracking
- Over 200 different validation failure types
- Timing validation and committee membership checks

### Architecture Change
```
BEFORE:
Test Input → Custom ValidationEngine → Simplified Validation → Expected Errors

AFTER: 
Test Input → Production Validator → Real Validation Logic → Mapped Error Messages
```

### Expected Outcomes
- **Authentic validation**: Tests use the same validation logic as production
- **Comprehensive coverage**: All edge cases and failure modes are tested
- **Reduced maintenance**: Single source of validation logic
- **Higher confidence**: Tests validate against real production behavior
- **Future-proof**: Automatic pickup of validation improvements

## Goals & Objectives

### Primary Goals
- **Replace custom validation** with production `message_validator::Validator`
- **Maintain test compatibility** by mapping `ValidationFailure` types to expected error strings
- **Achieve 100% test pass rate** with real validation logic

### Secondary Objectives
- **Simplify codebase** by removing custom validation implementation
- **Improve test reliability** by using production-tested validation
- **Enable future validation enhancements** without test code changes

## Solution Overview

### Approach
Replace the custom `ValidationEngine`, `MockSlashingDetector`, and related validation types with direct integration of the production `message_validator::Validator`. The validator requires several dependencies (database, slot clock, duties provider, task executor) that we'll set up using the same pattern as the existing fuzz tests.

### Key Components
1. **Validator Setup**: Initialize production `Validator` with required dependencies (database, slot clock, etc.)
2. **Message Processing**: Use `validator.validate()` instead of custom validation methods
3. **Error Mapping**: Convert `ValidationFailure` types to expected test error strings
4. **Test Integration**: Update test execution to use real validator while maintaining test structure

### Expected Outcomes
- All 13 validation tests continue to pass using real production validation logic
- Custom validation code (ValidationEngine, MockSlashingDetector, etc.) is removed
- Tests automatically benefit from future validation improvements
- Validation behavior matches production exactly

## Implementation Tasks

### CRITICAL IMPLEMENTATION RULES
1. **NO PLACEHOLDER CODE**: Every implementation must be production-ready and use real validation logic
2. **MAINTAIN TEST COMPATIBILITY**: All existing tests must continue to pass with identical behavior
3. **COMPLETE INTEGRATION**: Remove all custom validation code and replace with production validator
4. **AUTHENTIC VALIDATION**: Use real RSA signatures, database state, and production validation rules

### Visual Dependency Tree
```
anchor/spec_tests/
├── Cargo.toml (Task #0: Add message_validator and related dependencies)
├── src/
│   ├── ssv/
│   │   └── validation.rs (Task #2: Update test execution to use real validator)
│   ├── utils/
│   │   ├── mod.rs (Task #1: Remove custom validation exports)
│   │   ├── validation_engine.rs (Task #1: REMOVE - replaced by real validator) 
│   │   └── mock_slashing.rs (Task #1: REMOVE - replaced by real validator)
│   ├── types/
│   │   ├── mod.rs (Task #1: Remove custom validation type exports)
│   │   └── validation_types.rs (Task #1: REMOVE - replaced by ValidationFailure)
│   └── lib.rs (Task #1: Update module structure)
```

### Execution Plan

#### Group A: Dependencies and Setup (Execute all in parallel)
- [ ] **Task #0**: Add production validator dependencies
  - **File**: `Cargo.toml`
  - **Implements**: Add dependencies for message_validator, database, slot_clock, task_executor, duties_tracker, openssl, tempfile
  - **Dependencies**: 
    ```toml
    message_validator = { path = "../message_validator" }
    database = { path = "../database" }
    slot_clock = { path = "../slot_clock" }  
    task_executor = { path = "../task_executor" }
    duties_tracker = { path = "../duties_tracker" }
    openssl = { workspace = true }
    tempfile = { workspace = true }
    async-channel = { workspace = true }
    futures = { workspace = true }
    ```
  - **Context**: These are required for initializing the production Validator instance

#### Group B: Remove Custom Code and Setup Real Validator (Execute all in parallel after Group A)
- [ ] **Task #1**: Remove custom validation infrastructure and create real validator setup
  - **Files to REMOVE**: 
    - `src/utils/validation_engine.rs` 
    - `src/utils/mock_slashing.rs`
    - `src/types/validation_types.rs`
  - **Files to UPDATE**:
    - `src/utils/mod.rs` - Remove exports: `pub use validation_engine::ValidationEngine;` and `pub use mock_slashing::MockSlashingDetector;`
    - `src/types/mod.rs` - Remove exports: `pub use validation_types::*;`
    - `src/lib.rs` - Update module structure to remove validation_types
  - **New File**: `src/utils/real_validator.rs`
  - **Implements**:
    ```rust
    use std::{path::Path, sync::Arc, time::Duration};
    use message_validator::{Validator, ValidationResult, ValidationFailure};
    use database::NetworkDatabase;
    use slot_clock::{SlotClock, ManualSlotClock};
    use task_executor::TaskExecutor;
    use duties_tracker::MockDutiesProvider;
    use tempfile::tempdir;
    use openssl::rsa::Rsa;
    use types::Slot;

    // Test RSA key (same as used in fuzz tests)
    const TESTING_KEY: &str = include_str!("../../../message_validator/src/testing_key.pem");

    pub fn create_test_validator() -> Arc<Validator<ManualSlotClock, MockDutiesProvider>> {
        // Setup slot clock
        let slot_clock = ManualSlotClock::new(
            Slot::new(100), // Current slot matching ValidationContext::current_epoch
            Duration::from_secs(0),
            Duration::from_secs(12),
        );
        
        // Setup database with RSA key  
        let rsa = Rsa::private_key_from_pem(TESTING_KEY.as_bytes()).expect("Key is valid");
        let public_key = Rsa::from_public_components(
            rsa.n().to_owned().unwrap(),
            rsa.e().to_owned().unwrap(),
        ).unwrap();
        
        let tempdir = tempdir().unwrap();
        let file = tempdir.path().join("test_db.sqlite");
        let db = NetworkDatabase::new(&file, &public_key).expect("Database construction");

        // Setup duties provider
        let duties_provider = MockDutiesProvider::new();

        // Setup task executor
        let handle = tokio::runtime::Handle::current();
        let (_signal, exit) = async_channel::bounded(1);
        let (shutdown_tx, _) = futures::channel::mpsc::channel(1);
        let executor = TaskExecutor::new(handle, exit, shutdown_tx, "test_executor".into());

        // Create validator
        Arc::new(Validator::new(
            db.watch(),
            32,  // slots_per_epoch
            256, // epochs_per_sync_committee_period
            512, // sync_committee_size  
            Arc::new(duties_provider),
            slot_clock,
            &executor,
        ))
    }

    pub fn map_validation_failure_to_expected_error(failure: &ValidationFailure) -> String {
        match failure {
            // Map ValidationFailure variants to expected test error strings
            ValidationFailure::WrongDomain => "wrong domain".to_string(),
            ValidationFailure::UnknownValidator => "duty invalid: wrong validator index".to_string(),
            ValidationFailure::InvalidValidatorPubKey => "duty invalid: wrong validator pk".to_string(),
            ValidationFailure::WrongBeaconRoleType => "duty invalid: wrong beacon role type".to_string(),
            ValidationFailure::EarlySlotMessage { .. } => "duty invalid: duty epoch is into far future".to_string(),
            ValidationFailure::SlashableAttestation => "slashable attestation".to_string(),
            ValidationFailure::AttestationSourceGreaterThanTarget => "attestation data source >= target".to_string(),
            ValidationFailure::AttestationTargetInFarFuture => "attestation data target epoch is into far future".to_string(),
            ValidationFailure::InvalidValue => "invalid value".to_string(),
            ValidationFailure::PreDecodeFailure(msg) => format!("failed decoding consensus data: {}", msg),
            _ => format!("validation error: {:?}", failure),
        }
    }

    pub fn validate_message_with_real_validator(
        validator: &Validator<ManualSlotClock, MockDutiesProvider>,
        message_data: &[u8],
    ) -> Result<(), String> {
        match validator.validate(message_data) {
            ValidationResult::Success(_) => Ok(()),
            ValidationResult::PreDecodeFailure(failure) => {
                Err(map_validation_failure_to_expected_error(&failure))
            }
            ValidationResult::PostDecodeFailure(failure, _) => {
                Err(map_validation_failure_to_expected_error(&failure))
            }
        }
    }
    ```
  - **Exports**: `create_test_validator`, `validate_message_with_real_validator`, `map_validation_failure_to_expected_error`
  - **Context**: Replaces all custom validation infrastructure with production validator setup

#### Group C: Update Test Integration (Execute after Group B)
- [ ] **Task #2**: Update SSV validation tests to use real validator
  - **File**: `src/ssv/validation.rs`
  - **Remove imports**:
    ```rust
    use crate::types::{ValidationContext, SlashableSlots};
    use crate::utils::ValidationEngine;
    ```
  - **Add imports**:
    ```rust
    use crate::utils::{create_test_validator, validate_message_with_real_validator};
    use std::sync::Arc;
    use message_validator::Validator;
    use slot_clock::ManualSlotClock;
    use duties_tracker::MockDutiesProvider;
    ```
  - **Replace validation logic**:
    - Remove `ValidationContext` and `ValidationEngine` setup
    - Replace with `let validator = create_test_validator();`
    - Replace `engine.validate_by_role()` calls with `validate_message_with_real_validator()`
    - Remove runner_role parameter (production validator determines role from message content)
    - Remove duty_slot parameter (production validator uses message timestamp)
    - Remove slashable_slots JSON parsing (production validator has built-in slashing detection)
  - **Update methods**:
    ```rust
    fn run_single_validation(&self) -> bool {
        let validator = create_test_validator();
        let input = self.input.as_ref().unwrap();
        
        // Decode base64 input data
        let decoded_data = base64::engine::general_purpose::STANDARD
            .decode(input)
            .expect("Valid base64 in test data");
        
        let result = validate_message_with_real_validator(&validator, &decoded_data);
        let expected_error = self.expected_error.as_ref();
        
        match (&result, expected_error) {
            (Err(actual_error), Some(expected)) => {
                actual_error == expected || expected.is_empty()
            },
            (Ok(()), None) => true,
            (Ok(()), Some(ref e)) if e.is_empty() => true,
            _ => false,
        }
    }
    
    fn run_validation_subtest(&self, test: &ValidationSubTest) -> bool {
        let validator = create_test_validator();
        
        // Decode base64 input data
        let decoded_data = base64::engine::general_purpose::STANDARD
            .decode(&test.input)
            .expect("Valid base64 in test data");
            
        let result = validate_message_with_real_validator(&validator, &decoded_data);
        
        match (&result, &test.expected_error) {
            (Err(actual_error), expected) => {
                actual_error == expected || expected.is_empty()
            },
            (Ok(()), expected) => expected.is_empty(),
        }
    }
    ```
  - **Context**: Completely replaces custom validation with production validator while maintaining test structure and expected error matching

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
- Tasks should be run in parallel, unless there are dependencies, using subtasks, to avoid context bloat

### Progress Tracking
The checkboxes above represent the authoritative status of each task. Keep them updated as you work.