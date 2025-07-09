# QBFT Hash Mismatch Diagnostic and Resolution Plan

## Executive Summary
> **Problem Statement**: 6 out of 8 QBFT spec tests are failing with tree hash root mismatches despite correct field values and justification serialization. The core issue is fundamental differences in SSZ serialization implementations between Rust (ethereum_ssz) and Go (fastssz), resulting in different tree hash calculations for identical data structures.
>
> **Proposed Solution**: Implement a comprehensive diagnostic and resolution system to identify, isolate, and fix the SSZ serialization differences causing hash mismatches. This includes byte-level comparison tools, SSZ compatibility layers, and systematic field-by-field validation to ensure exact Go compatibility.
>
> **Technical Approach**: Create diagnostic tools to compare raw SSZ bytes between implementations, implement custom SSZ encoding methods that match Go behavior exactly, and establish a validation framework to ensure future compatibility.
>
> **Expected Outcomes**: All 8 QBFT create message tests pass with matching tree hash roots, establishing complete compatibility between Rust and Go SSZ implementations for QBFT messages.

## Goals & Objectives
### Primary Goals
- **Fix all 6 failing QBFT spec tests**: Achieve 8/8 passing tests with matching tree hash roots
- **Establish SSZ compatibility**: Create exact byte-for-byte compatibility with Go fastssz implementation

### Secondary Objectives
- **Build diagnostic infrastructure**: Create tools for future SSZ compatibility debugging
- **Ensure maintainability**: Establish testing patterns to prevent future regressions
- **Document SSZ differences**: Create clear documentation of Rust vs Go SSZ differences

## Solution Overview
### Approach
The solution addresses the root cause of SSZ serialization differences between Rust and Go implementations by creating diagnostic tools to identify exact byte-level differences, implementing compatibility layers to match Go behavior precisely, and establishing validation frameworks to ensure ongoing compatibility.

### Key Components
1. **SSZ Byte Comparison Tools**: Direct byte-level comparison between Rust and Go serialization
2. **Custom SSZ Implementation**: Drop-in replacement that matches Go fastssz behavior exactly
3. **Field-Level Diagnostics**: Granular analysis of each struct field's serialization
4. **Tree Hash Validation**: Verification that identical data produces identical hashes

### Data Flow
```
JSON Test Input → Deserialize Fields → Create QBFT Message → Custom SSZ Encode → Tree Hash → Compare with Expected
                                                       ↓
                              Diagnostic Tools → Byte Comparison → Identify Differences → Fix Implementation
```

### Expected Outcomes
- All QBFT create message tests pass with matching tree hash roots
- Complete SSZ serialization compatibility with Go implementation
- Robust diagnostic tools for future compatibility issues
- Clear documentation of SSZ implementation differences

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
│   ├── diagnostics.rs (Task #0: SSZ diagnostic and comparison tools)
│   ├── mod.rs (Task #2: Enhanced QBFT message creation with diagnostics)
│   └── create_message.rs (Task #3: Improved test validation and debugging)
│
├── common/ssv_types/src/
│   ├── message.rs (Task #1: Custom SSZ implementation matching Go fastssz)
│   └── ssz_compat.rs (Task #1: SSZ compatibility layer and utilities)
│
└── common/qbft/src/
    └── lib.rs (Task #4: Update justification handling with new SSZ methods)
```

### Execution Plan

#### Group A: Foundation - Diagnostic Infrastructure (Execute first)
- [ ] **Task #0**: Create SSZ diagnostic and byte comparison tools
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `diagnostics.rs`
  - **Imports**: 
    - `use ssv_types::message::SignedSSVMessage;`
    - `use ssz::Encode;`
    - `use hex;`
    - `use std::fs;`
  - **Implements**:
    - **Function `compare_ssz_bytes(rust_msg: &SignedSSVMessage, go_bytes: &[u8]) -> ComparisonResult`**:
      - Compares Rust SSZ output with Go SSZ bytes
      - Returns detailed byte-by-byte differences
      - Identifies offset table differences
      - Highlights field boundary mismatches
    - **Function `analyze_ssz_structure(bytes: &[u8]) -> SszStructure`**:
      - Parses SSZ bytes to understand offset table
      - Maps field boundaries and sizes
      - Validates SSZ container structure
    - **Function `debug_field_serialization(msg: &SignedSSVMessage) -> FieldDebugInfo`**:
      - Serializes each field individually
      - Compares field-level SSZ output
      - Identifies which fields cause size differences
    - **Function `load_go_comparison_data(test_name: &str) -> Option<Vec<u8>>`**:
      - Loads Go SSZ bytes from test files
      - Handles test name mapping and file paths
      - Provides fallback for missing comparison data
    - **Struct `ComparisonResult`**: Detailed comparison results with byte differences
    - **Struct `SszStructure`**: Parsed SSZ structure information
    - **Struct `FieldDebugInfo`**: Per-field serialization debugging data
  - **Integration**: Used by test framework and QBFT message creation for debugging
  - **Testing**: Include unit tests for each diagnostic function

#### Group B: SSZ Compatibility Implementation (Execute after Group A)
- [ ] **Task #1**: Implement custom SSZ encoding matching Go fastssz behavior
  - **Folder**: `anchor/common/ssv_types/src/`
  - **Files**: `message.rs`, `ssz_compat.rs`
  - **Imports**:
    - `use ssz::{Encode, Decode, DecodeError};`
    - `use ssz_types::VariableList;`
    - `use crate::consensus::*;`
    - `use std::convert::TryInto;`
  - **Implements in `ssz_compat.rs`**:
    - **Trait `GoCompatibleSsz`**: Interface for Go-compatible SSZ serialization
      ```rust
      trait GoCompatibleSsz {
          fn marshal_ssz_go_compat(&self) -> Vec<u8>;
          fn unmarshal_ssz_go_compat(bytes: &[u8]) -> Result<Self, DecodeError> where Self: Sized;
      }
      ```
    - **Function `encode_variable_list_go_compat<T>(list: &VariableList<T, N>) -> Vec<u8>`**:
      - Encodes VariableList using Go fastssz offset calculation
      - Matches exact Go behavior for nested variable lists
      - Handles empty lists correctly
    - **Function `encode_signature_list_go_compat(signatures: &SignatureList) -> Vec<u8>`**:
      - Specifically handles signature list encoding
      - Matches Go's compact signature representation
      - Optimizes for the 13-signature, 256-byte structure
    - **Function `calculate_go_offsets(field_sizes: &[usize]) -> Vec<u32>`**:
      - Calculates offset table using Go fastssz algorithm
      - Ensures exact offset matching with Go implementation
      - Handles variable-length field padding correctly
  - **Implements in `message.rs`**:
    - **Update `SignedSSVMessage` with custom encoding**:
      ```rust
      impl GoCompatibleSsz for SignedSSVMessage {
          fn marshal_ssz_go_compat(&self) -> Vec<u8> {
              // Custom implementation matching Go fastssz exactly
              // Uses go-compatible offset calculation
              // Matches signature list encoding exactly
              // Produces identical byte output to Go
          }
      }
      ```
    - **Update `encode_without_full_data` method**:
      - Use `marshal_ssz_go_compat()` instead of `as_ssz_bytes()`
      - Ensure exact Go fastssz compatibility
      - Verify 484-byte output matches Go exactly
    - **Add `encode_with_go_compat(&self) -> Vec<u8>` method**:
      - Alternative to standard SSZ encoding
      - Uses Go-compatible serialization throughout
      - Provides drop-in replacement for standard encoding
  - **Integration**: Replaces standard SSZ encoding in QBFT message creation
  - **Testing**: Verify exact byte-for-byte matching with Go test cases

#### Group C: Enhanced Message Creation (Execute after Group B)
- [ ] **Task #2**: Update QBFT message creation with Go-compatible SSZ encoding
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `mod.rs`
  - **Imports**:
    - `use ssv_types::ssz_compat::GoCompatibleSsz;`
    - `use super::diagnostics::*;`
  - **Implements**:
    - **Update `create_message` method**:
      ```rust
      pub fn create_message(
          &self,
          message_type: QbftMessageType,
          data_hash: Hash256,
          round: Option<Round>,
          round_change_justifications: Vec<SignedSSVMessage>,
          prepare_justifications: Vec<SignedSSVMessage>,
      ) -> UnsignedWrappedQbftMessage {
          // Use Go-compatible SSZ encoding for all components
          // Apply diagnostics to validate encoding
          // Ensure justifications use marshal_ssz_go_compat()
      }
      ```
    - **Update `sign` method**:
      - Use `marshal_ssz_go_compat()` for message body serialization
      - Apply diagnostic validation before returning result
      - Log detailed comparison with expected Go output
    - **Add `validate_ssz_compatibility` function**:
      - Validates that created messages match Go SSZ output
      - Runs diagnostic comparison automatically
      - Returns detailed mismatch information for debugging
  - **Integration**: Core message creation used by all QBFT tests with enhanced validation
  - **Testing**: Verify all created messages use Go-compatible encoding

#### Group D: Test Framework Enhancement (Execute after Group C)
- [ ] **Task #3**: Enhance test validation with comprehensive SSZ debugging
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `create_message.rs`
  - **Imports**:
    - `use super::diagnostics::*;`
    - `use ssv_types::ssz_compat::GoCompatibleSsz;`
  - **Implements**:
    - **Update `run` method in `CreateMessageTest`**:
      ```rust
      fn run(&self) -> bool {
          // Create message using Go-compatible encoding
          // Run comprehensive SSZ diagnostics
          // Compare byte-by-byte with Go output
          // Provide detailed mismatch analysis
          // Return true only if exact hash match
      }
      ```
    - **Update `detailed_comparison` method**:
      - Add SSZ byte comparison using diagnostic tools
      - Show exact byte differences when hashes don't match
      - Identify specific fields causing mismatches
      - Provide actionable debugging information
    - **Add `validate_go_ssz_compatibility` method**:
      ```rust
      fn validate_go_ssz_compatibility(&self, rust_msg: &SignedSSVMessage) -> Result<(), String> {
          // Load Go comparison data if available
          // Run comprehensive byte comparison
          // Validate offset table structure
          // Check field boundary alignment
          // Return detailed error information
      }
      ```
    - **Update error reporting**:
      - Include SSZ diagnostic information in test output
      - Show byte-level differences alongside hash mismatches
      - Provide clear guidance for fixing SSZ compatibility issues
  - **Integration**: Enhanced test debugging for all QBFT create message tests
  - **Testing**: Verify diagnostic output provides actionable debugging information

#### Group E: Justification Integration (Execute after Group D)
- [ ] **Task #4**: Update QBFT library to use Go-compatible SSZ methods
  - **Folder**: `anchor/common/qbft/src/`
  - **File**: `lib.rs`
  - **Imports**:
    - `use ssv_types::ssz_compat::GoCompatibleSsz;`
  - **Implements**:
    - **Update justification serialization in `new_unsigned_message`**:
      ```rust
      // Replace existing justification encoding
      let round_change_justification_vec: Vec<VariableList<u8, _>> = round_change_justification
          .into_iter()
          .map(|msg| {
              let without_full_data = msg.without_full_data();
              VariableList::from(without_full_data.marshal_ssz_go_compat()) // Use Go-compatible encoding
          })
          .collect();
      
      let prepare_justification_vec: Vec<VariableList<u8, _>> = prepare_justification
          .into_iter()
          .map(|msg| {
              let without_full_data = msg.without_full_data();
              VariableList::from(without_full_data.marshal_ssz_go_compat()) // Use Go-compatible encoding
          })
          .collect();
      ```
    - **Update justification validation in `validate_justifications`**:
      - Use `GoCompatibleSsz::unmarshal_ssz_go_compat` for decoding justifications
      - Ensure validation uses same encoding as creation
      - Maintain consistency throughout justification handling
    - **Add validation for SSZ compatibility**:
      - Verify all justification operations use Go-compatible encoding
      - Add debug logging for justification size validation
      - Ensure 484-byte justification encoding matches Go exactly
  - **Integration**: Core QBFT justification handling with Go SSZ compatibility
  - **Testing**: Verify all justification operations produce Go-compatible output
  - **Result**: Justifications serialized to exact Go byte length and structure

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