# QBFT Controller Test Debugging & Fixes Implementation Plan

## Executive Summary
> **Problem Statement**: After successfully integrating the complete `validate_consensus_message` function, QBFT controller tests are failing due to signature validation mismatches and timing validation issues that weren't present with semantic-only validation.
>
> **Proposed Solution**: Fix the mock infrastructure to properly support the complete validation pipeline by aligning test keys with spec data and implementing precise timing simulation that meets the 50ms tolerance requirements.
>
> **Technical Approach**: 
> 1. Replace randomly generated RSA keys with predefined TestKeySet keys that match spec test signatures
> 2. Implement slot-aligned timing simulation with microsecond precision
> 3. Validate duty and state management work correctly with complete validation
>
> **Expected Outcomes**: All 53 QBFT controller tests pass with complete validation pipeline active, proving the integration is production-ready and maintains spec compliance.

## Goals & Objectives
### Primary Goals
- **100% Controller Test Pass Rate**: All 53 QBFT controller tests pass with complete validation pipeline
- **Signature Validation Success**: Test signatures validate correctly against predefined operator keys
- **Timing Tolerance Compliance**: All messages arrive within 50ms tolerance window

### Secondary Objectives
- **Maintain Spec Compliance**: Ensure validation behavior matches Go reference implementation expectations
- **Performance Optimization**: Complete validation should not significantly impact test execution time
- **Infrastructure Robustness**: Mock infrastructure supports all validation scenarios

## Solution Overview
### Approach
Replace the mock infrastructure components that generate random test data with implementations that use the existing TestKeySet infrastructure and implement precise timing simulation aligned with slot boundaries.

### Key Components
1. **ValidationAdapter**: Replace random key generation with TestKeySet integration for signature validation
2. **MockSlotClock**: Implement precise timing simulation that aligns with message slot requirements
3. **Timing Calculation**: Calculate realistic `received_at` timestamps within validation tolerance windows
4. **Error Mapping**: Ensure new validation errors are properly mapped to Go spec test format

### Architecture Diagram
```
Test Spec JSON → SignedSSVMessage → ValidationAdapter → Complete Validation Pipeline
     ↓                    ↓              ↓                         ↓
  RSA Sigs        TestKeySet Keys    Slot-Aligned        validate_consensus_message
(predefined)       (matching)        Timing             (with all checks)
```

### Data Flow
```
Message Creation → Key Validation → Timing Check → Semantic Check → State Update
     ↓                ↓               ↓             ↓                ↓
TestKeySet RSA → Signature Match → 50ms Window → Quorum/Round → QBFT State
```

### Expected Outcomes
- All 53 controller tests pass with complete validation active
- Signature validation succeeds for all multi-signer messages
- Timing validation passes within 50ms tolerance for all message scenarios
- Complete validation pipeline proven to work in production-like test environment

## Implementation Tasks

### CRITICAL IMPLEMENTATION RULES
1. **NO PLACEHOLDER CODE**: Every implementation must be production-ready. NEVER write "TODO", "in a real implementation", or similar placeholders unless explicitly requested by the user.
2. **CROSS-DIRECTORY TASKS**: Group related changes across directories into single tasks to ensure consistency. Never create isolated changes that require follow-up work in sibling directories.
3. **COMPLETE IMPLEMENTATIONS**: Each task must fully implement its feature including all consumers, type updates, and integration points.
4. **DETAILED SPECIFICATIONS**: Each task must include EXACTLY what to implement, including specific functions, types, and integration points to avoid "breaking change" confusion.
5. **CONTEXT AWARENESS**: Each task is part of a larger system - specify how it connects to other parts.
6. **MAKE BREAKING CHANGES**: Unless explicitly requested by the user, you MUST make breaking changes.

### Visual Dependency Tree
```
src/qbft/
├── validation_adapter.rs (Task #1: Fix signature validation with TestKeySet integration)
├── unified_test_adapter.rs (Task #2: Update timing and error handling integration)
└── controller_test.rs (Task #3: Enhance error mapping for new validation failures)

src/utils/
└── test_keys.rs (Task #0: Verify TestKeySet infrastructure supports validation needs)
```

### Execution Plan

#### Group A: Foundation Verification (Execute in parallel)
- [x] **Task #0**: Verify TestKeySet Infrastructure Compatibility
  - **Folder**: `src/utils/`
  - **File**: `test_keys.rs` (analysis only, no changes needed)
  - **Research Task**: Verify that `TestKeySet::four_share_set()` provides the correct RSA keys matching spec test signatures
  - **Validation**: 
    - Confirm operator ID 1-4 RSA keys exist and are accessible as public keys
    - Verify key format compatibility with OpenSSL Rsa<Public> type
    - Test that extracted public keys can validate signatures created by corresponding private keys
  - **Deliverable**: Confirmation that TestKeySet infrastructure is ready for ValidationAdapter integration
  - **Integration**: Provides foundation for Task #1 signature validation fix

#### Group B: Core Validation Fixes (Execute in parallel after Group A)
- [x] **Task #1**: Fix ValidationAdapter Signature Validation with TestKeySet Integration
  - **Folder**: `src/qbft/`
  - **File**: `validation_adapter.rs`
  - **Problem**: Currently generates random RSA keys instead of using TestKeySet keys that match spec test signatures
  - **Imports**:
    - `use crate::utils::test_keys::TestKeySet`
    - Keep existing imports for message_validator integration
  - **Implementation**:
    - **Modify `ValidationAdapter::new()`**:
      ```rust
      pub fn new(committee: IndexSet<OperatorId>) -> Self {
          let test_keys = TestKeySet::four_share_set();
          Self {
              committee_info: CommitteeInfo { /* existing */ },
              test_keys, // Add TestKeySet to struct
              slot_clock: MockSlotClock::new(1),
              duties_provider: Arc::new(MockDutiesProvider),
              slots_per_epoch: 32,
          }
      }
      ```
    - **Replace Random Key Generation in `validate_signed_message()`**:
      ```rust
      // Replace lines 150-158 random key generation with:
      let operator_pks: Vec<Rsa<Public>> = msg.operator_ids().iter().map(|&operator_id| {
          let private_key = self.test_keys.operator_keys.get(&operator_id)
              .expect("Operator key not found in TestKeySet");
          // Extract public key from private key
          Rsa::from_public_components(
              private_key.n().to_owned().expect("Failed to get modulus"),
              private_key.e().to_owned().expect("Failed to get exponent"),
          ).expect("Failed to create public key")
      }).collect();
      ```
    - **Implement Slot-Aligned Timing**:
      ```rust
      // Calculate proper received_at based on message slot
      let consensus_message = QbftMessage::from_ssz_bytes(msg.ssv_message().data())
          .map_err(|e| ValidationFailure::UndecodableMessageData(e))?;
      let target_slot = Slot::new(consensus_message.height);
      let slot_start = slot_start_time(target_slot, self.slot_clock.clone())
          .map_err(|_| ValidationFailure::SlotStartTimeNotFound { slot: target_slot })?;
      let received_at = slot_start + Duration::from_millis(25); // Within 50ms tolerance
      ```
  - **Struct Changes**: Add `test_keys: TestKeySet` field to `ValidationAdapter`
  - **Exports**: No changes to exports, maintains same public API
  - **Integration**: Uses TestKeySet from utils, integrates with existing UnifiedTestAdapter
  - **Context**: This fixes the primary cause of validation failures (signature mismatches)

- [x] **Task #2**: Update MockSlotClock for Precise Timing Alignment  
  - **Folder**: `src/qbft/`
  - **File**: `validation_adapter.rs` (MockSlotClock implementation)
  - **Problem**: Current timing causes "early by 59s" errors due to genesis time misalignment
  - **Implementation**:
    - **Fix `MockSlotClock::new()`**:
      ```rust
      impl MockSlotClock {
          pub fn new(current_slot: u64) -> Self {
              let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
                  .unwrap_or(Duration::from_secs(0));
              // Set genesis so current_slot aligns with current time
              let genesis_time = now.saturating_sub(Duration::from_secs(current_slot * 12));
              Self { current_slot, genesis_time }
          }
          
          // Add genesis_time field and update all trait methods to use it
          fn start_of(&self, slot: Slot) -> Option<Duration> {
              Some(self.genesis_time + Duration::from_secs(slot.as_u64() * 12))
          }
      }
      ```
    - **Add `genesis_time` field**: `pub struct MockSlotClock { current_slot: u64, genesis_time: Duration }`
    - **Update all trait methods**: Use `genesis_time` as reference point for slot calculations
  - **Integration**: Works with ValidationAdapter's slot-aligned timing calculation
  - **Context**: Ensures timing validation passes within 50ms tolerance

#### Group C: Error Handling & Integration (Execute in parallel after Group B)
- [x] **Task #3**: Enhance Error Mapping for Complete Validation Failures
  - **Folder**: `src/qbft/`
  - **File**: `controller_test.rs`
  - **Problem**: New validation failures from complete pipeline may not map correctly to Go spec test error format
  - **Implementation**:
    - **Expand `map_validation_failure_to_go_format()`** with new patterns:
      ```rust
      match validation_failure {
          // Add new complete validation error mappings
          ValidationFailure::SlotStartTimeNotFound { slot } => {
              format!("slot start time not found for slot {}", slot.as_u64())
          },
          ValidationFailure::EarlySlotMessage { got } => {
              format!("early slot message: {}", got)
          },
          ValidationFailure::LateSlotMessage { got } => {
              format!("late slot message: {}", got)  
          },
          ValidationFailure::SignatureVerificationFailed { reason } => {
              format!("msg signature invalid: {}", reason)
          },
          // Keep existing mappings
          ValidationFailure::SignatureVerification => {
              "msg signature invalid: crypto/rsa: verification error".to_string()
          },
          // ... existing patterns
      }
      ```
    - **Add timing-specific error handling**: Map slot timing errors to appropriate Go error messages
    - **Maintain backward compatibility**: Keep all existing error mappings for semantic validation
  - **Exports**: No changes to public API
  - **Integration**: Used by all controller test error handling
  - **Context**: Ensures proper error messages when complete validation catches new failure types

- [x] **Task #4**: Verify Complete Integration and Run Validation Tests
  - **Folder**: `src/qbft/`
  - **Files**: `unified_test_adapter.rs`, `validation_adapter.rs`, `controller_test.rs`
  - **Implementation**:
    - **Test Complete Integration**:
      ```bash
      cargo test test_qbft_controller -- --nocapture 2>&1 | head -n 200
      ```
    - **Verify Key Integration**: Confirm signature validation passes for multi-signer messages
    - **Validate Timing Precision**: Ensure all messages fall within 50ms tolerance
    - **Check Error Mapping**: Verify new validation errors map correctly to Go format
    - **Performance Validation**: Ensure complete validation doesn't significantly slow tests
  - **Success Criteria**:
    - All 53 controller tests pass OR show expected validation errors matching Go spec
    - No signature validation failures due to key mismatches
    - No timing validation failures outside expected edge cases
    - Error messages match Go spec test expectations
  - **Documentation**: Update any validation-related documentation
  - **Context**: Final integration verification ensuring all components work together

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
- Tasks should be run in parallel, unless there are dependencies, using subtasks, to avoid context bloat.

### Progress Tracking
The checkboxes above represent the authoritative status of each task. Keep them updated as you work.