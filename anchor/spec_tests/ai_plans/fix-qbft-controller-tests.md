# QBFT Controller Tests Fix Implementation Plan

## Executive Summary

> **Problem**: Our QBFT controller tests are failing because we're over-validating compared to the Go controller tests. The Go controller tests only perform minimal validation (message decodability + identifier matching), while our current implementation uses full semantic validation including round checks, quorum validation, and signature verification.
>
> **Solution**: Align our test validation pipeline with the Go controller test expectations by implementing a minimal validation mode that matches their validation level exactly. This involves creating a controller-test-specific validation path that bypasses semantic validation layers while still using our existing validation infrastructure.
>
> **Technical Approach**: Create a minimal validation adapter that only performs the validation steps that the Go controller tests expect, allowing us to test our core QBFT logic without getting blocked by validation rules that don't apply in the test environment.
>
> **Expected Outcomes**: Achieve 100% test compliance with Go controller specification tests while maintaining our existing validation infrastructure for production use.

## Goals & Objectives

### Primary Goals
- **Test Compliance**: Achieve 100% pass rate on QBFT controller specification tests
- **Infrastructure Reuse**: Leverage existing validation infrastructure without creating test-specific validation logic
- **Validation Alignment**: Match Go controller test validation level exactly (minimal validation only)

### Secondary Objectives
- **Maintain Production Validation**: Keep existing full validation pipeline intact for production use
- **Error Message Mapping**: Ensure test error messages match Go controller test expectations
- **Performance Optimization**: Minimize validation overhead during test execution

## Solution Overview

### Approach
Create a validation mode switch that allows the test adapter to use minimal validation (matching Go controller tests) while production code continues to use full validation. This is achieved by exposing a basic validation function that only checks what the Go controller tests check.

### Key Components
1. **Minimal Validation Function**: Extract basic validation logic (decodability + identifier matching) from existing functions
2. **Validation Mode Configuration**: Allow test adapter to specify validation level
3. **Error Message Alignment**: Map minimal validation errors to Go controller test format

### Architecture Diagram
```
Test Input → MinimalValidation → QBFT Processing → Test Output
             (Go Controller Level)
                     ↓
Production → SemanticValidation → FullValidation → QBFT Processing
             (Current Level)      (Signature + Duty)
```

### Data Flow
```
Controller Test → ValidationAdapter → minimal_validate_consensus_message → QBFT Logic
                                   (decodability + identifier only)
                                           ↓
Production Use → MessageValidator → validate_consensus_message → QBFT Logic
                                  (full validation pipeline)
```

### Expected Outcomes
- **100% controller test compliance**: All 53 QBFT controller tests pass
- **Maintained production validation**: No changes to existing validation pipeline
- **Accurate error reporting**: Error messages match Go controller test expectations

## Implementation Tasks

### CRITICAL IMPLEMENTATION RULES
1. **NO NEW VALIDATION LOGIC**: Reuse existing validation infrastructure components
2. **MINIMAL VALIDATION ONLY**: Only validate what Go controller tests validate
3. **PRODUCTION UNCHANGED**: Keep existing validation pipeline intact for production use
4. **ERROR MESSAGE ALIGNMENT**: Map validation errors to exact Go controller test format

### Visual Dependency Tree
```
anchor/message_validator/src/
├── consensus_message.rs (Task #0: Extract minimal validation function)
└── lib.rs (Task #0: Export minimal validation function)

anchor/spec_tests/src/qbft/
├── validation_adapter.rs (Task #1: Switch to minimal validation mode)
├── unified_test_adapter.rs (Task #2: Update validation integration)
└── controller_test.rs (Task #2: Update error message mapping)
```

### Execution Plan

#### Group A: Extract Minimal Validation (Execute Task #0)
- [ ] **Task #0**: Extract minimal validation function from existing infrastructure
  - Folder: `anchor/message_validator/src/`
  - File: `consensus_message.rs`
  - **Implement**: New function `minimal_validate_consensus_message` that only does:
    ```rust
    pub fn minimal_validate_consensus_message(
        signed_ssv_message: &SignedSSVMessage,
    ) -> Result<QbftMessage, ValidationFailure> {
        // 1. Decode message to QbftMessage
        let consensus_message = QbftMessage::from_ssz_bytes(
            signed_ssv_message.ssv_message().data(),
        ).map_err(ValidationFailure::UndecodableMessageData)?;
        
        // 2. Validate identifier match (only check Go controller tests do)
        if consensus_message.identifier != VariableList::from(signed_ssv_message.ssv_message().msg_id()) {
            return Err(ValidationFailure::MismatchedIdentifier {
                got: hex::encode(&*consensus_message.identifier),
                want: hex::encode(signed_ssv_message.ssv_message().msg_id()),
            });
        }
        
        Ok(consensus_message)
    }
    ```
  - **Export**: Add to `lib.rs` exports: `pub use crate::consensus_message::minimal_validate_consensus_message;`
  - **Context**: This matches exactly what Go controller tests validate - just decodability and identifier matching
  - **Integration**: Used by ValidationAdapter instead of semantic validation

#### Group B: Update Test Validation (Execute all in parallel after Group A)
- [ ] **Task #1**: Switch ValidationAdapter to minimal validation mode
  - Folder: `anchor/spec_tests/src/qbft/`
  - File: `validation_adapter.rs`
  - **Imports**: 
    ```rust
    use message_validator::minimal_validate_consensus_message;
    // Remove: validate_consensus_message_semantics import
    ```
  - **Modify**: `validate_signed_message()` method to use minimal validation:
    ```rust
    pub fn validate_signed_message(&self, msg: &SignedSSVMessage) -> Result<(), ValidationFailure> {
        // Basic pre-validation checks only
        if msg.operator_ids().is_empty() {
            return Err(ValidationFailure::NoSigners);
        }
        
        // Use minimal validation that matches Go controller tests
        let _qbft_message = minimal_validate_consensus_message(msg)?;
        
        Ok(())
    }
    ```
  - **Context**: This aligns our validation with Go controller test expectations
  - **Integration**: Used by UnifiedTestAdapter for message validation

- [ ] **Task #2**: Update test integration and error mapping
  - Folder: `anchor/spec_tests/src/qbft/`
  - Files: `unified_test_adapter.rs`, `controller_test.rs`
  - **Unified Test Adapter Changes**:
    - Update `process_message()` to handle minimal validation results
    - Ensure error propagation works with simplified validation
  - **Controller Test Changes**:
    - Update `map_error_to_go_format()` to handle minimal validation errors
    - Focus on identifier mismatch and decode errors (primary failure modes)
    - Remove mappings for semantic validation errors that won't occur
  - **Context**: Complete the integration of minimal validation into test execution
  - **Integration**: Final step to achieve Go controller test compliance

#### Group C: Verification and Testing (Execute after Group B)
- [ ] **Task #3**: Verify complete integration and test compliance
  - **Run Tests**: Execute all QBFT controller tests to verify 100% pass rate
  - **Regression Check**: Ensure production validation pipeline still works
  - **Error Message Verification**: Confirm error messages match Go controller expectations
  - **Performance Check**: Verify minimal validation improves test execution speed

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