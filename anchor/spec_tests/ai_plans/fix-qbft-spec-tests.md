# QBFT Spec Tests Fix Implementation Plan

## Executive Summary
> **Problem Statement**: 6 out of 8 QBFT spec tests are failing with tree hash root mismatches due to incorrect full_data field handling and justification serialization differences between the Rust and Go implementations.
>
> **Proposed Solution**: Fix the data transformation logic in QBFT message creation, implement proper justification serialization without full_data, and correct the Value-to-root field mapping to match the Go specification exactly.
>
> **Technical Approach**: 
> 1. Implement custom SSZ serialization for SignedSSVMessage to exclude full_data in justifications
> 2. Fix the full_data field construction logic based on message type and justification presence
> 3. Correct the Value field transformation to properly derive the root field
> 4. Add proper data extraction from justifications for complex proposals
>
> **Expected Outcomes**: All 8 QBFT create message tests pass with matching tree hash roots, establishing compatibility between Rust and Go implementations.

## Goals & Objectives
### Primary Goals
- **Fix all failing QBFT spec tests**: Achieve 8/8 passing tests with matching tree hash roots
- **Implement correct justification serialization**: Match Go behavior by excluding full_data from justifications entirely

### Secondary Objectives
- **Establish spec test compatibility**: Create a robust foundation for future QBFT test additions
- **Improve code maintainability**: Clean up data transformation logic with proper abstractions

## Solution Overview
### Approach
The solution addresses three core issues: (1) incorrect full_data length caused by using the wrong data source, (2) improper justification serialization that includes empty full_data instead of excluding it entirely, and (3) missing logic to extract proposal data from justifications in complex message scenarios.

### Key Components
1. **SignedSSVMessage Serialization**: Custom SSZ implementation to conditionally exclude full_data
2. **Message Creation Logic**: Enhanced data transformation based on message type and justifications
3. **Test Data Processing**: Proper Value field interpretation and justification data extraction

### Data Flow
```
JSON Test Input → Deserialize Value & Justifications → Extract Proposal Data → Create QBFT Message → Sign Message → Calculate Tree Hash → Compare with Expected
```

### Expected Outcomes
- All QBFT create message tests pass with matching tree hash roots
- Justifications are properly serialized without full_data field
- Message creation logic correctly handles both simple and complex proposals

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
anchor/
├── common/ssv_types/src/
│   └── message.rs (Task #0: Custom SSZ serialization for SignedSSVMessage)
│
├── spec_tests/src/qbft/
│   ├── mod.rs (Task #1: Fix message creation and data transformation logic)
│   └── create_message.rs (Task #2: Add justification data extraction helpers)
│
└── common/qbft/src/
    └── lib.rs (Task #3: Update QBFT to use new serialization methods)
```

### Execution Plan

#### Group A: Foundation - Custom Serialization (Execute first)
- [x] **Task #0**: Implement custom SSZ serialization for SignedSSVMessage
  - **Folder**: `anchor/common/ssv_types/src/`
  - **File**: `message.rs`
  - **Imports**: 
    - `use ssz::{Encode, Decode, SszEncoder, SszDecoder, DecodeError};`
    - `use ethereum_ssz_derive::TreeHash;`
  - **Implements**:
    - Remove `Encode, Decode` from the derive macro on `SignedSSVMessage` struct
    - Custom `impl Encode for SignedSSVMessage` that always includes full_data
    - Custom `impl Decode for SignedSSVMessage` for compatibility
    - New method `encode_without_full_data(&self) -> Vec<u8>` that serializes only signatures, operator_ids, and ssv_message
    - New method `from_ssz_bytes_without_full_data(bytes: &[u8]) -> Result<Self, DecodeError>` for deserialization
  - **Integration**: Used by QBFT justification serialization and test framework
  - **Breaking Changes**: Changes internal serialization behavior but maintains API compatibility
  - **SSZ Layout for encode_without_full_data()**:
    ```rust
    // Offset 0: signatures (VariableList)
    // Offset N: operator_ids (VariableList) 
    // Offset M: ssv_message (fixed size)
    // Note: full_data field is completely omitted
    ```

#### Group B: Message Creation Logic (Execute after Group A)
- [x] **Task #1**: Fix QBFT message creation and data transformation
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `mod.rs`
  - **Imports**:
    - `use ssv_types::message::SignedSSVMessage;`
    - `use types::Hash256;`
  - **Implements**:
    - **Fix `create_message()` method (lines 67-94)**:
      - For proposals: Check if justifications contain full_data, if so extract it (27 bytes), otherwise use Value field (32 bytes)
      - For other message types: Use Value field as root, leave full_data empty
    - **Add helper function**:
      ```rust
      fn extract_proposal_data_from_justifications(
          round_change_justifications: &[SignedSSVMessage],
          prepare_justifications: &[SignedSSVMessage]
      ) -> Option<Vec<u8>> {
          // Extract full_data from first available justification
          // Return None if no justifications or all have empty full_data
      }
      ```
    - **Update `sign()` method (lines 96-117)** to use new serialization for message body
  - **Integration**: Core message creation used by all QBFT tests
  - **Data Flow**: JSON Value → extract justification data → construct proper full_data → create message → sign → tree hash

- [x] **Task #2**: Add justification data extraction and test helpers
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `create_message.rs`
  - **Imports**:
    - `use base64::decode;`
    - `use serde_json;`
  - **Implements**:
    - **Add method to `CreateMessageTest`**:
      ```rust
      fn get_expected_full_data(&self) -> Vec<u8> {
          // Logic to determine what full_data should be based on message type and justifications
          // For proposals with justifications: extract from justifications
          // For simple proposals: use Value field
          // For other types: empty
      }
      ```
    - **Enhanced debugging in `detailed_comparison()`**:
      - Add comparison of expected vs actual full_data construction
      - Show step-by-step data transformation
      - Verify justification processing logic
    - **Add validation helpers**:
      ```rust
      fn validate_justification_data(&self) -> Result<(), String> {
          // Validate that justifications have expected format
          // Check base64 decoding works correctly
          // Verify data lengths match expectations
      }
      ```
  - **Integration**: Enhanced test debugging and validation for message creation tests

#### Group C: QBFT Integration (Execute after Group B)
- [x] **Task #3**: Update QBFT to use new justification serialization
  - **Folder**: `anchor/common/qbft/src/`
  - **File**: `lib.rs`
  - **Imports**: No new imports needed
  - **Implements**:
    - **Update justification serialization (lines 934-944)**:
      ```rust
      let round_change_justification_vec: Vec<VariableList<u8, _>> = round_change_justification
          .into_iter()
          .map(|msg| VariableList::from(msg.encode_without_full_data())) // Use new method
          .collect();
      ```
    - **Update prepare justification serialization (lines 945-955)**:
      ```rust
      let prepare_justification_vec: Vec<VariableList<u8, _>> = prepare_justification
          .into_iter()
          .map(|msg| VariableList::from(msg.encode_without_full_data())) // Use new method  
          .collect();
      ```
    - **Add validation** that justification sizes match Go implementation expectations
  - **Integration**: Core QBFT message construction used throughout the consensus layer
  - **Result**: Justifications will be serialized to 27 bytes instead of 32 bytes, matching Go behavior

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
- Run tests after each task to verify progress

### Progress Tracking
The checkboxes above represent the authoritative status of each task. Keep them updated as you work.