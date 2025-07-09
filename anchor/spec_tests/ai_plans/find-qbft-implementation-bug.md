# QBFT Implementation Bug Detection Plan

## Executive Summary
> **Problem Statement**: 6 out of 8 QBFT spec tests are failing with tree hash mismatches, but both `ethereum_ssz` (Rust) and `fastssz` (Go) are correct SSZ implementations. The bug must be in our specific implementation logic - either in how we construct the message fields, handle data types, or serialize specific components.
>
> **Proposed Solution**: Systematically isolate and identify the exact implementation bug by comparing our field-by-field construction logic with the Go implementation, validating data type handling, and tracing the exact differences in our message creation process.
>
> **Technical Approach**: Create precise diagnostic tools to compare field values, data types, and construction logic between our Rust implementation and the expected Go behavior. Focus on finding the specific code bug rather than SSZ library differences.
>
> **Expected Outcomes**: Identify and fix the exact implementation bug causing hash mismatches, resulting in all 8 QBFT tests passing.

## Goals & Objectives
### Primary Goals
- **Identify the exact implementation bug**: Find the specific code issue causing hash mismatches
- **Fix the root cause**: Resolve the bug without changing SSZ libraries

### Secondary Objectives
- **Validate our understanding**: Confirm that SSZ libraries are not the issue
- **Prevent future bugs**: Establish better validation patterns

## Solution Overview
### Approach
Since both SSZ implementations are correct, the issue must be in our specific code that constructs, handles, or processes the message data before it gets to the SSZ layer. We need to systematically check every step of our message construction process.

### Key Components
1. **Field Construction Validation**: Verify each field is built correctly
2. **Data Type Verification**: Ensure all types match Go exactly  
3. **Message Assembly Logic**: Check how we combine fields into messages
4. **Serialization Path Tracing**: Follow the exact path from JSON to hash

### Expected Outcomes
- Identify the specific bug in our implementation
- All QBFT tests pass after fixing the bug
- Clear understanding of what was wrong

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
├── spec_tests/src/qbft/
│   ├── field_validator.rs (Task #0: Field-by-field validation tools)
│   ├── mod.rs (Task #2: Debug message construction process)
│   └── create_message.rs (Task #3: Enhanced test debugging with exact comparisons)
│
└── common/ssv_types/src/
    └── message.rs (Task #1: Validate SignedSSVMessage construction and types)
```

### Execution Plan

#### Group A: Diagnostic Foundation (Execute first)
- [ ] **Task #0**: Create field-by-field validation and debugging tools
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `field_validator.rs`
  - **Imports**:
    - `use ssv_types::message::SignedSSVMessage;`
    - `use ssv_types::consensus::*;`
    - `use ssz::{Encode, Decode};`
    - `use tree_hash::TreeHash;`
    - `use hex;`
  - **Implements**:
    - **Function `validate_message_construction(test_name: &str, rust_msg: &SignedSSVMessage, expected_hash: Hash256) -> ValidationReport`**:
      - Validates each field individually against expected values
      - Compares field-by-field SSZ encoding
      - Checks data type consistency
      - Returns detailed report of any discrepancies
    - **Function `compare_field_encoding<T: Encode>(field: &T, field_name: &str) -> FieldValidation`**:
      - Encodes individual field and validates its SSZ output
      - Compares field size and structure
      - Identifies field-specific encoding issues
    - **Function `validate_signatures_field(signatures: &SignatureList) -> FieldValidation`**:
      - Specifically validates signature list construction
      - Checks signature count, length, and encoding
      - Validates against expected signature structure
    - **Function `validate_operator_ids_field(operator_ids: &VariableList<OperatorId, U13>) -> FieldValidation`**:
      - Validates operator ID list construction
      - Checks ID values and list structure
      - Ensures proper OperatorId encoding
    - **Function `validate_ssv_message_field(ssv_message: &SSVMessage) -> FieldValidation`**:
      - Validates the nested SSVMessage structure
      - Checks MsgType, MessageId, and data fields
      - Validates QbftMessage nested encoding
    - **Function `validate_full_data_field(full_data: &VariableList<u8, SSVMessageFullDataLen>) -> FieldValidation`**:
      - Validates full_data field construction
      - Checks length and content against expected values
      - Identifies full_data handling issues
    - **Struct `ValidationReport`**: Comprehensive validation results
    - **Struct `FieldValidation`**: Per-field validation results with detailed diagnostics
  - **Integration**: Used by test framework to validate every message field
  - **Testing**: Validate each field independently to isolate issues

#### Group B: Type and Construction Validation (Execute after Group A)
- [ ] **Task #1**: Validate SignedSSVMessage construction and data types
  - **Folder**: `anchor/common/ssv_types/src/`
  - **File**: `message.rs`
  - **Imports**: No new imports needed
  - **Implements**:
    - **Add debug validation methods to `SignedSSVMessage`**:
      ```rust
      impl SignedSSVMessage {
          pub fn validate_construction(&self) -> ConstructionValidation {
              // Validate each field is constructed correctly
              // Check that all types match expected Go equivalents
              // Verify field relationships and constraints
              // Return detailed validation results
          }
          
          pub fn debug_field_values(&self) -> FieldDebugInfo {
              // Output detailed information about each field
              // Include sizes, types, and content summaries
              // Provide hex dumps of critical data
              // Format for easy comparison with Go output
          }
          
          pub fn compare_with_expected(&self, expected_fields: &ExpectedFields) -> ComparisonResult {
              // Compare our constructed fields with expected values
              // Identify exact field-level differences
              // Provide actionable debugging information
              // Focus on finding construction bugs
          }
      }
      ```
    - **Add validation for `new_from_vecs` constructor**:
      - Validate that the constructor builds fields correctly
      - Check signature handling and OperatorId assignment
      - Verify SSVMessage and full_data construction
      - Ensure no data corruption during construction
    - **Add type consistency validation**:
      - Verify OperatorId type matches Go OperatorID exactly
      - Check that SignatureList structure matches Go [][]byte
      - Validate SSVMessage type alignment with Go *SSVMessage
      - Ensure VariableList bounds match Go ssz-max values
    - **Add debug output for message creation**:
      ```rust
      pub fn debug_creation_process(
          signatures: Vec<Signature>, 
          operator_ids: Vec<OperatorId>, 
          ssv_message: SSVMessage, 
          full_data: Vec<u8>
      ) -> CreationDebugInfo {
          // Track each step of message construction
          // Validate input types and conversions
          // Check intermediate field creation
          // Return detailed construction trace
      }
      ```
  - **Integration**: Enhanced debugging for all SignedSSVMessage creation
  - **Testing**: Validate that message construction produces expected field values

#### Group C: Message Creation Process Debugging (Execute after Group B)
- [ ] **Task #2**: Debug and validate QBFT message construction process
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `mod.rs`
  - **Imports**:
    - `use super::field_validator::*;`
  - **Implements**:
    - **Update `create_message` method with comprehensive debugging**:
      ```rust
      pub fn create_message(
          &self,
          message_type: QbftMessageType,
          data_hash: Hash256,
          round: Option<Round>,
          round_change_justifications: Vec<SignedSSVMessage>,
          prepare_justifications: Vec<SignedSSVMessage>,
      ) -> UnsignedWrappedQbftMessage {
          // Log input parameters and validate them
          // Debug justification processing step-by-step
          // Validate QBFT message construction
          // Check UnsignedSSVMessage creation
          // Trace full_data assignment logic
          // Return with full debug information
      }
      ```
    - **Update `sign` method with validation**:
      ```rust
      fn sign(
          &self,
          unsigned: UnsignedWrappedQbftMessage,
          private_key: &PKey<Private>,
      ) -> SignedSSVMessage {
          // Debug the signing process
          // Validate message serialization before signing
          // Check signature creation and format
          // Validate final message construction
          // Compare with expected field values
          // Log detailed construction steps
      }
      ```
    - **Add `debug_justification_processing` function**:
      - Trace how justifications are processed and serialized
      - Validate that justifications are handled correctly
      - Check for any data corruption in justification handling
      - Compare with expected justification behavior
    - **Add `validate_message_assembly` function**:
      - Validate how components are assembled into final message
      - Check for any field assignment bugs
      - Verify data type conversions are correct
      - Ensure no data is lost or corrupted during assembly
  - **Integration**: Core message creation with comprehensive debugging
  - **Testing**: Validate each step of message construction process

#### Group D: Test Framework Enhancement with Exact Comparison (Execute after Group C)
- [ ] **Task #3**: Enhance test validation with exact field-by-field comparison
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `create_message.rs`
  - **Imports**:
    - `use super::field_validator::*;`
  - **Implements**:
    - **Update `run` method with comprehensive validation**:
      ```rust
      fn run(&self) -> bool {
          // Create message with full debugging enabled
          // Run field-by-field validation against expected values
          // Compare actual vs expected for every field
          // Identify exact discrepancies causing hash mismatch
          // Log detailed comparison results
          // Return true only if all validations pass
      }
      ```
    - **Update `detailed_comparison` method**:
      ```rust
      fn detailed_comparison(
          &self,
          go_state: &CreateMessageTest,
          rust_msg: &SignedSSVMessage,
          go_file_path: &str,
      ) {
          // Add field-by-field validation
          // Compare signatures, operator_ids, ssv_message, full_data individually
          // Check for construction bugs in each field
          // Identify the exact field causing hash mismatch
          // Provide specific guidance on fixing the issue
      }
      ```
    - **Add `extract_expected_fields_from_go` function**:
      - Parse Go test files to extract expected field values
      - Create ExpectedFields structure for comparison
      - Handle JSON deserialization carefully
      - Ensure no data loss in parsing
    - **Add `identify_field_discrepancy` function**:
      ```rust
      fn identify_field_discrepancy(&self, rust_msg: &SignedSSVMessage) -> Option<FieldDiscrepancy> {
          // Compare each field with expected Go values
          // Find the exact field that doesn't match
          // Provide specific information about the discrepancy
          // Return actionable debugging information
      }
      ```
    - **Update error reporting with precise information**:
      - Show exactly which field is causing the hash mismatch
      - Include expected vs actual values for that field
      - Provide specific guidance on what to fix
      - Focus on implementation bugs rather than SSZ differences
  - **Integration**: Enhanced test debugging that identifies exact implementation bugs
  - **Testing**: Verify that field-by-field comparison identifies the root cause

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
- Focus on finding the exact implementation bug

### Progress Tracking
The checkboxes above represent the authoritative status of each task. Keep them updated as you work.