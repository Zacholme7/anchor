# SSV Spec Test Parsing Fix Implementation Plan

## Executive Summary

The current SSV spec test implementation has multiple critical issues preventing proper test discovery and execution. All SSV tests are failing with "No tests found" errors because:

1. **Incorrect file discovery path**: Tests are located in `ssv-spec/ssv/spectest/generate/tests/` but the system looks in `ssv-spec/ssv/spectest/{directory}/`
2. **Missing test registration**: SSV tests are not registered in the `TEST_LOADERS` system, bypassing the standard architecture
3. **JSON structure mismatches**: Rust structs don't match the actual JSON file structure
4. **Missing test categories**: Several test types exist in JSON files but have no corresponding Rust structs

The solution involves fixing the file discovery logic, aligning JSON structures with Rust types, and properly integrating SSV tests into the existing registration system.

## Goals & Objectives

### Primary Goals
- **Fix test discovery**: Enable all 200+ SSV test files to be properly discovered and loaded
- **Align data structures**: Update Rust structs to match actual JSON file structure
- **Standardize architecture**: Integrate SSV tests into the existing `TEST_LOADERS` registration system

### Secondary Objectives
- **Improve maintainability**: Use consistent patterns across all test types
- **Enable test execution**: Prepare foundation for actual test implementation
- **Support new test categories**: Add missing test types found in JSON files

## Solution Overview

### Approach
Fix the SSV test system by correcting file discovery paths, updating JSON-to-Rust structure mappings, and properly registering all test types in the existing architecture.

### Key Components
1. **File Discovery Fix**: Update path resolution to point to the correct test directory
2. **JSON Structure Alignment**: Modify Rust structs to match actual JSON file format
3. **Test Registration**: Add all SSV test types to the `TEST_LOADERS` system
4. **Filename Pattern Matching**: Implement proper file-to-test-type mapping logic

### Expected Outcomes
- All SSV test categories successfully discover their respective test files
- Approximately 200+ SSV test files become available for execution
- SSV tests follow the same architecture patterns as QBFT and Types tests
- Test execution framework is ready for actual implementation

## Implementation Tasks

### CRITICAL IMPLEMENTATION RULES
1. **NO PLACEHOLDER CODE**: Every implementation must be production-ready with proper error handling
2. **CROSS-DIRECTORY TASKS**: Group related changes across directories into single tasks to ensure consistency
3. **COMPLETE IMPLEMENTATIONS**: Each task must fully implement its feature including all integration points
4. **DETAILED SPECIFICATIONS**: Each task includes exactly what to implement with specific functions and types

### Visual Dependency Tree

```
anchor/spec_tests/
├── src/
│   ├── lib.rs (Task #5: Update test discovery and registration)
│   ├── ssv/
│   │   ├── mod.rs (Task #3: Add new test types and create_ssv_test updates)
│   │   ├── committee.rs (Task #1: Fix committee test structures)
│   │   ├── message_processing.rs (Task #1: Fix message processing structures)
│   │   ├── validation.rs (Task #1: Fix validation structures)
│   │   ├── partial_signatures.rs (Task #1: Fix partial signature structures)
│   │   ├── duty_execution.rs (Task #1: Fix duty execution structures)
│   │   ├── controller.rs (Task #1: Fix controller structures)
│   │   ├── runner_construction.rs (Task #2: Create new test category)
│   │   ├── sync_committee_aggregator.rs (Task #2: Create new test category)
│   │   └── new_duty.rs (Task #2: Create new test category)
│   └── utils/
│       └── ssv_test_discovery.rs (Task #4: Create test discovery utilities)
└── ssv-spec/ssv/spectest/generate/tests/ (Task #0: Verify test files exist)
```

### Execution Plan

#### Group A: Foundation Analysis (Execute in parallel)
- [x] **Task #0**: Verify test file structure and mapping
  - **Files**: Analysis of `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/ssv-spec/ssv/spectest/generate/tests/`
  - **Implements**: File listing categorization by prefix patterns
  - **Purpose**: Confirm test file organization and validate filename-to-test-type mapping
  - **Deliverables**: Complete mapping of all test files to their respective categories
  - **Integration**: Provides foundation for Tasks #1-4

#### Group B: JSON Structure Fixes (Execute in parallel after Group A)
- [x] **Task #1**: Fix existing SSV test structures to match JSON format
  - **Files**: 
    - `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/committee.rs`
    - `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/message_processing.rs`
    - `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/validation.rs`
    - `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/partial_signatures.rs`
    - `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/duty_execution.rs`
    - `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/controller.rs`
  - **Implements**: Updated struct definitions that match actual JSON structure
  - **Key Changes**:
    - **Committee**: Add `Committee`, `Input`, `PostDutyCommitteeRoot`, `OutputMessages`, `BeaconBroadcastedRoots`, `ExpectedError` fields
    - **Message Processing**: Change `Share` to `HashMap<String, Share>`, add missing fields
    - **Validation**: Add `Type`, `Network`, `RunnerRole`, `DutySlot`, `Input`, `SlashableSlots`, `AnyError` fields
    - **Partial Signatures**: Add `Type`, `Documentation`, `ExpectedResult` fields
    - **Duty Execution**: Update to match `newduty.*` JSON structure
    - **Controller**: Verify structure matches actual JSON files
  - **Exports**: Updated test structs with correct serde annotations
  - **Integration**: Provides properly typed structs for test loading

- [x] **Task #2**: Create missing test category implementations
  - **Files**: 
    - `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/runner_construction.rs`
    - `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/sync_committee_aggregator.rs`
    - `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/new_duty.rs`
    - `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/multi_committee.rs`
    - `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/multi_validation.rs`
  - **Implements**: Complete test struct definitions for missing categories
  - **Key Structures**:
    - **RunnerConstruction**: `SsvRunnerConstructionTest` with `Shares`, `RoleError` fields
    - **SyncCommitteeAggregator**: `SsvSyncCommitteeAggregatorTest` with `ProofRootsMap`, `OperatorID` fields
    - **NewDuty**: `SsvNewDutyTest` with duty execution structure
    - **MultiCommittee**: `SsvMultiCommitteeTest` with `Tests` array
    - **MultiValidation**: `SsvMultiValidationTest` with `Tests` array
  - **Exports**: All new test structs implementing `SpecTest` trait
  - **Integration**: Extends test category coverage to match all JSON files

#### Group C: Type System Updates (Execute in parallel after Group B)
- [x] **Task #3**: Update SSV module organization and test type enumeration
  - **Files**: `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/ssv/mod.rs`
  - **Implements**: 
    - Extended `SsvSpecTestType` enum with new variants:
      ```rust
      #[derive(Eq, PartialEq, Hash, Debug, Clone)]
      pub enum SsvSpecTestType {
          Controller,
          MessageProcessing,
          MultiMessageProcessing,
          Committee,
          MultiCommittee,
          PartialSignatures,
          Validation,
          MultiValidation,
          DutyExecution,
          RunnerConstruction,
          SyncCommitteeAggregator,
          NewDuty,
      }
      ```
    - Updated `create_ssv_test` function to handle all test types with proper JSON deserialization
    - Updated `directory_name()` method to return correct paths (currently unused but for consistency)
  - **Exports**: Complete enum coverage and test creation function
  - **Integration**: Provides type system foundation for test registration

- [x] **Task #4**: Create SSV test discovery utilities
  - **Files**: `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/utils/ssv_test_discovery.rs`
  - **Implements**: 
    - `determine_ssv_test_type(filename: &str) -> Option<SsvSpecTestType>` function with filename pattern matching:
      ```rust
      pub fn determine_ssv_test_type(filename: &str) -> Option<SsvSpecTestType> {
          if filename.starts_with("committee.MultiCommitteeSpecTest_") {
              Some(SsvSpecTestType::MultiCommittee)
          } else if filename.starts_with("committee.CommitteeSpecTest_") {
              Some(SsvSpecTestType::Committee)
          } else if filename.starts_with("tests.MultiMsgProcessingSpecTest_") {
              Some(SsvSpecTestType::MultiMessageProcessing)
          } else if filename.starts_with("tests.MsgProcessingSpecTest_") {
              Some(SsvSpecTestType::MessageProcessing)
          } else if filename.starts_with("partialsigcontainer.") {
              Some(SsvSpecTestType::PartialSignatures)
          } else if filename.starts_with("valcheck.MultiSpecTest_") {
              Some(SsvSpecTestType::MultiValidation)
          } else if filename.starts_with("valcheck.SpecTest_") {
              Some(SsvSpecTestType::Validation)
          } else if filename.starts_with("runnerconstruction.") {
              Some(SsvSpecTestType::RunnerConstruction)
          } else if filename.starts_with("synccommitteeaggregator.") {
              Some(SsvSpecTestType::SyncCommitteeAggregator)
          } else if filename.starts_with("newduty.") {
              Some(SsvSpecTestType::NewDuty)
          } else {
              None
          }
      }
      ```
  - **Exports**: Test type determination utility for file discovery
  - **Integration**: Used by main test discovery logic to categorize files

#### Group D: Test System Integration (Execute sequentially after Group C)
- [x] **Task #5**: Update main test discovery and registration system
  - **Files**: `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/src/lib.rs`
  - **Implements**: 
    - **Test Registration**: Add all SSV test types to `TEST_LOADERS` map:
      ```rust
      static TEST_LOADERS: LazyLock<Loaders> = register_test_loaders!(
          // Existing tests...
          TimeoutTest,
          CreateMessageTest,
          BeaconVoteEncodingTest,
          // ... existing types tests ...
          
          // ADD: SSV tests
          SsvControllerTest,
          SsvMessageProcessingTest,
          SsvMultiMessageProcessingTest,
          SsvCommitteeTest,
          SsvMultiCommitteeTest,
          SsvPartialSignatureTest,
          SsvValidationTest,
          SsvMultiValidationTest,
          SsvDutyExecutionTest,
          SsvRunnerConstructionTest,
          SsvSyncCommitteeAggregatorTest,
          SsvNewDutyTest,
      );
      ```
    - **Path Fix**: Update `Display` implementation for `SpecTestType` to use correct path:
      ```rust
      SpecTestType::Ssv(_) => {
          write!(f, "ssv-spec/ssv/spectest/generate/tests")
      }
      ```
    - **Discovery Logic**: Replace special SSV handling with standard filename pattern matching:
      ```rust
      // Remove special SSV case, use standard discovery with filename pattern matching
      // that calls determine_ssv_test_type() for SSV files
      ```
  - **Key Changes**:
    - Remove special SSV case from `run_tests` function
    - Add SSV tests to standard registration system
    - Use filename pattern matching for SSV test discovery
    - Maintain consistent architecture across all test types
  - **Exports**: Fully integrated test discovery system
  - **Integration**: Final integration point that makes all SSV tests discoverable

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