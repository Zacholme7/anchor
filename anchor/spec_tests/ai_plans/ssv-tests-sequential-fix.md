# SSV Tests Sequential Fix Implementation Plan

## Executive Summary

The SSV spec test framework is currently partially functional, with 2 out of 7 test categories working properly. The remaining 5 categories fail due to JSON parsing issues including missing fields, type mismatches, and structure inconsistencies. This plan addresses these issues one-by-one, ensuring each test category is completely fixed and passing before moving to the next.

The approach prioritizes fixing the easiest issues first (simple field additions) before tackling more complex structural problems. Each fix will use proper type parsing (not generic Value types) while maintaining no-op implementations that return true after successful parsing.

## Goals & Objectives

### Primary Goals
- **Sequential Fix Strategy**: Fix each test category completely before moving to the next
- **Proper Type Parsing**: Ensure all JSON fields are parsed into appropriate Rust types, not generic Value
- **Complete Test Coverage**: All 7 SSV test categories should discover, parse, and execute successfully

### Secondary Objectives
- **Maintainable Code**: Use consistent patterns and proper error handling
- **Foundation for Implementation**: Prepare structured data for actual test logic implementation
- **Comprehensive Coverage**: Handle all JSON variations and edge cases

## Solution Overview

### Approach
Fix SSV tests in order of complexity, starting with simple field additions and progressing to complex structural changes. Each category will be completely resolved before moving to the next.

### Key Components
1. **Field Additions**: Add missing optional fields to handle JSON variations
2. **Type Corrections**: Fix field types to match actual JSON structure
3. **Structure Alignment**: Modify complex nested structures to match JSON format
4. **Validation Logic**: Ensure all parsing succeeds with proper error handling

### Data Flow
```
JSON Files → Filename Pattern → Test Type → Struct Parsing → No-op Execution → Success
```

### Expected Outcomes
- All 7 SSV test categories pass completely
- Approximately 156 SSV test files parse successfully
- Foundation ready for actual test implementation
- Consistent error handling across all test types

## Implementation Tasks

### CRITICAL IMPLEMENTATION RULES
1. **NO PLACEHOLDER CODE**: Every implementation must be production-ready with proper error handling
2. **SEQUENTIAL APPROACH**: Complete each test category fully before moving to the next
3. **PROPER TYPE PARSING**: Use appropriate Rust types, not generic Value types
4. **NO-OP IMPLEMENTATIONS**: All test run() methods return true after successful parsing
5. **COMPLETE VERIFICATION**: Each task must include testing to verify success

### Visual Dependency Tree

```
anchor/spec_tests/src/ssv/
├── validation.rs (Task #1: Fix MultiSpecTest Network field) ✅
├── partial_signatures.rs (Task #2: Fix null value handling)
├── controller.rs (Task #3: Add missing Type field)
├── duty_execution.rs (Task #4: Add missing Type field)
├── message_processing.rs (Task #5: Add missing Type field + structure fixes)
├── committee.rs (Task #6: Fix Committee structure + Type field)
└── message_processing.rs (Task #7: Fix MultiMessageProcessing complex issues)
```

### Execution Plan

#### Phase 1: Simple Field Fixes (Execute sequentially)
- [x] **Task #1**: Fix Validation MultiSpecTest parsing
  - **Files**: `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/validation.rs`
  - **Problem**: `valcheck.MultiSpecTest_*` files missing `Network` field
  - **Solution**: Add `Network` field as optional in ValidationSubTest or create separate struct
  - **Implements**: 
    - Update `ValidationSubTest` to include optional `Network` field
    - Or create separate `MultiValidationTest` struct with proper field handling
    - Ensure all valcheck.* files parse successfully
  - **Test Command**: `cargo test ssv_tests::test_ssv_validation -- --nocapture`
  - **Success Criteria**: All 13 validation test files parse and load successfully ✅
  - **Integration**: Ready for actual validation logic implementation

- [ ] **Task #2**: Fix Partial Signatures null value handling
  - **Files**: `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/partial_signatures.rs`
  - **Problem**: `partialsigcontainer.PartialSigContainerTest_PartialSigContainer_duplicate.json` has null where string expected
  - **Solution**: Make string fields optional where null values are possible
  - **Implements**:
    - Review JSON structure to identify nullable fields
    - Update `SsvPartialSignatureTest` to handle null values properly
    - Add proper Option<String> types for nullable fields
  - **Test Command**: `cargo test ssv_tests::test_ssv_partial_signatures -- --nocapture`
  - **Success Criteria**: All 5 partial signature test files parse and load successfully
  - **Integration**: Ready for actual partial signature logic implementation

#### Phase 2: Missing Type Field Fixes (Execute sequentially)
- [ ] **Task #3**: Fix Controller missing Type field
  - **Files**: `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/controller.rs`
  - **Problem**: Missing `Type` field in controller test files
  - **Solution**: Add optional `Type` field to `SsvControllerTest` struct
  - **Implements**:
    - Add `#[serde(rename = "Type")] pub test_type: Option<String>` to `SsvControllerTest`
    - Ensure all controller test files parse successfully
    - Verify no other missing fields in controller JSON structure
  - **Test Command**: `cargo test ssv_tests::test_ssv_controller -- --nocapture`
  - **Success Criteria**: All controller test files parse and load successfully
  - **Integration**: Ready for actual controller logic implementation

- [ ] **Task #4**: Fix Duty Execution missing Type field
  - **Files**: `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/duty_execution.rs`
  - **Problem**: Missing `Type` field in duty execution test files
  - **Solution**: Add optional `Type` field to `SsvDutyExecutionTest` struct
  - **Implements**:
    - Add `#[serde(rename = "Type")] pub test_type: Option<String>` to `SsvDutyExecutionTest`
    - Ensure all newduty.* test files parse successfully
    - Verify sub-test structure matches JSON format
  - **Test Command**: `cargo test ssv_tests::test_ssv_duty_execution -- --nocapture`
  - **Success Criteria**: All 11 duty execution test files parse and load successfully
  - **Integration**: Ready for actual duty execution logic implementation

- [x] **Task #5**: Fix Message Processing missing Type field
  - **Files**: `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/message_processing.rs`
  - **Problem**: Missing `Type` field in message processing test files
  - **Solution**: Add optional `Type` field to `SsvMessageProcessingTest` struct
  - **Implements**:
    - Add `#[serde(rename = "Type")] pub test_type: Option<String>` to `SsvMessageProcessingTest`
    - Ensure all tests.MsgProcessingSpecTest_* files parse successfully
    - Verify BaseRunnerConfig structure matches JSON format
  - **Test Command**: `cargo test ssv_tests::test_ssv_message_processing -- --nocapture`
  - **Success Criteria**: All single message processing test files parse and load successfully
  - **Integration**: Ready for actual message processing logic implementation

#### Phase 3: Complex Structure Fixes (Execute sequentially)
- [x] **Task #6**: Fix Committee structure and Type field
  - **Files**: `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/committee.rs`
  - **Problem**: `Committee` field structure mismatch + missing `Type` field
  - **Solution**: Fix `Committee` field to match JSON structure with objects containing `OperatorID` and `SSVOperatorPubKey`
  - **Implements**:
    - Update `Committee` struct to handle object format instead of simple Vec<u64>
    - Add proper fields for `OperatorID` and `SSVOperatorPubKey`
    - Add optional `Type` field to `SsvCommitteeTest`
    - Create proper operator representation matching JSON structure
  - **Test Command**: `cargo test ssv_tests::test_ssv_committee -- --nocapture`
  - **Success Criteria**: All 19 committee test files parse and load successfully
  - **Integration**: Ready for actual committee logic implementation

- [x] **Task #7**: Fix Multi Message Processing complex issues
  - **Files**: `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/message_processing.rs`
  - **Problem**: Missing `validator_pubkey` field + base64 strings where sequences expected
  - **Solution**: Add missing field and fix base64 decoding for byte sequences
  - **Implements**:
    - Add `validator_pubkey` field to appropriate struct
    - Fix base64 string parsing to convert to Vec<u8> where needed
    - Add custom deserializers for base64 fields if necessary
    - Ensure all tests.MultiMsgProcessingSpecTest_* files parse successfully
  - **Test Command**: `cargo test ssv_tests::test_ssv_multi_message_processing -- --nocapture`
  - **Success Criteria**: All multi message processing test files parse and load successfully
  - **Integration**: Ready for actual multi message processing logic implementation

#### Phase 4: Comprehensive Verification (Execute after all fixes)
- [x] **Task #8**: Final verification and cleanup
  - **Files**: All SSV test files
  - **Purpose**: Ensure all 7 test categories work correctly together
  - **Implements**:
    - Run complete SSV test suite
    - Verify all 156 test files parse successfully
    - Clean up any remaining unused imports or dead code
    - Document any remaining known issues
  - **Test Command**: `cargo test ssv_tests -- --nocapture`
  - **Success Criteria**: All 7 SSV test categories pass completely
  - **Integration**: Complete foundation ready for actual test implementation

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
- **SEQUENTIAL EXECUTION**: Complete each task fully before moving to the next
- **VERIFY SUCCESS**: Each task must include testing to confirm it works

### Progress Tracking
The checkboxes above represent the authoritative status of each task. Keep them updated as you work.