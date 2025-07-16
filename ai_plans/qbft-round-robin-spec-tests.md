# QBFT Round Robin Spec Tests Implementation Plan

## Executive Summary

The current round robin test implementation in `anchor/spec_tests/src/qbft/round_robin.rs` is fundamentally incorrect. It attempts to run complex consensus scenarios with message processing, while the actual Go specification tests a simple mathematical proposer selection algorithm. This implementation plan involves a complete rewrite to match the Go test structure and implement proper round-robin proposer validation.

### Problem Statement
The current implementation treats round robin as a consensus test with message processing, error handling, and state validation. However, the Go tests validate a pure algorithmic function that determines which operator should be the proposer for a given height and round combination.

### Proposed Solution
Complete rewrite of the round robin test to implement:
1. Simple algorithmic validation of proposer selection
2. Direct testing against expected height/round/proposer combinations
3. Proper JSON deserialization matching Go test structure
4. Clean, idiomatic Rust code without unnecessary complexity

### Technical Approach
- Remove all consensus-related code (QbftTestAdapter, message processing, state validation)
- Implement the round-robin proposer selection algorithm using QBFT crate types
- Create simple validation loop that tests algorithm against expected results
- Use proper JSON deserialization for Go test compatibility

### Expected Outcomes
- Round robin tests execute correctly and match Go test behavior
- Algorithm validates proposer selection fairness across different committee sizes
- Tests pass for all committee configurations (4, 7, 10, 13 members)
- Clean, maintainable code that follows Rust best practices

## Goals & Objectives

### Primary Goals
- **Correct Implementation**: Replace incorrect consensus-based test with proper algorithmic validation
- **Go Compatibility**: Match exact behavior and structure of Go round robin tests
- **Algorithm Validation**: Ensure round-robin proposer selection is fair and deterministic

### Secondary Objectives
- **Code Quality**: Write clean, idiomatic Rust code without unnecessary complexity
- **Performance**: Efficient execution of 10,000+ height/round combinations per test
- **Maintainability**: Clear structure that's easy to understand and modify

## Solution Overview

### Approach
Complete rewrite of the round robin test implementation to match Go specification structure. The new implementation will be a simple algorithmic test that validates proposer selection across different heights and rounds.

### Key Components
1. **RoundRobinTest Struct**: New data structure matching Go JSON format
2. **Proposer Selection Algorithm**: Implementation of the round-robin algorithm
3. **Test Validation Loop**: Simple iteration through height/round combinations
4. **JSON Deserialization**: Proper parsing of Go test files

### Algorithm Implementation
The round-robin proposer selection follows this formula:
```
first_round_index = height % committee_size
proposer_index = (first_round_index + round - 1) % committee_size
proposer = committee[proposer_index]
```

### Data Flow
```
JSON Test File → Deserialization → Test Execution → Algorithm Validation → Pass/Fail
```

### Expected Outcomes
- **Algorithmic Correctness**: Round-robin proposer selection matches Go implementation
- **Test Compatibility**: All existing Go test files pass without modification
- **Performance**: Fast execution of large test suites
- **Code Quality**: Clean, maintainable Rust implementation

## Implementation Tasks

### CRITICAL IMPLEMENTATION RULES
1. **NO PLACEHOLDER CODE**: Every implementation must be production-ready
2. **COMPLETE REWRITE**: Remove all existing consensus-related code
3. **ALGORITHM FOCUS**: Implement simple mathematical proposer selection
4. **GO COMPATIBILITY**: Match exact Go test structure and behavior
5. **CLEAN CODE**: Write idiomatic Rust with proper error handling

### Visual Dependency Tree
```
anchor/spec_tests/src/qbft/
├── round_robin.rs (Task #2: Complete rewrite with algorithm implementation)
├── adapter/types.rs (Task #1: Add round robin types if needed)
├── mod.rs (Task #3: Update exports and test registration)
└── ../../../common/qbft/ (Task #0: Verify QBFT crate imports)
```

### Execution Plan

#### Group A: Foundation (Execute all in parallel)
- [x] **Task #0**: Verify QBFT crate dependencies and imports
  - **Context**: Ensure all necessary types are available from QBFT crate
  - **Validation**: Check that OperatorId, Round, InstanceHeight are properly imported
  - **Imports**: Verify DefaultLeaderFunction and related utilities are accessible
  - **Dependencies**: Confirm IndexSet<OperatorId> is available for committee representation
  - **Output**: Confirmed list of available types and any missing dependencies

#### Group B: Core Implementation (Execute after Group A)
- [x] **Task #1**: Implement round robin test data structure
  - **File**: `anchor/spec_tests/src/qbft/round_robin.rs`
  - **Replace**: Entire existing RoundRobinTest struct and all related code
  - **Implement**: 
    ```rust
    #[derive(Debug, Clone, Deserialize)]
    pub struct RoundRobinTest {
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "Type")]
        pub test_type: String,
        #[serde(rename = "Documentation")]
        pub documentation: String,
        #[serde(rename = "Share")]
        pub share: SpecTestCommitteeMember,
        #[serde(rename = "Heights")]
        pub heights: Vec<u64>,
        #[serde(rename = "Rounds")]
        pub rounds: Vec<u64>,
        #[serde(rename = "Proposers")]
        pub proposers: Vec<u64>,
    }
    ```
  - **Imports**: 
    ```rust
    use serde::{Deserialize, Deserializer};
    use ssv_types::OperatorId;
    use qbft::InstanceHeight;
    use super::adapter::types::SpecTestCommitteeMember;
    ```
  - **Context**: This replaces the incorrect message-based structure with proper algorithmic test data
  - **Validation**: Struct fields match Go JSON test format exactly

- [x] **Task #2**: Implement round-robin proposer selection algorithm
  - **File**: `anchor/spec_tests/src/qbft/round_robin.rs`
  - **Location**: Add as function in the same file
  - **Implement**:
    ```rust
    fn round_robin_proposer(committee: &[OperatorId], height: u64, round: u64) -> OperatorId {
        let first_round_index = if height == 0 {
            0
        } else {
            (height as usize) % committee.len()
        };
        
        let index = (first_round_index + (round - 1) as usize) % committee.len();
        committee[index]
    }
    ```
  - **Context**: This is the core algorithm that determines proposer selection
  - **Validation**: Algorithm matches Go implementation exactly
  - **Edge Cases**: Handle empty committee, zero height, round boundaries

- [x] **Task #3**: Implement SpecTest trait for RoundRobinTest
  - **File**: `anchor/spec_tests/src/qbft/round_robin.rs`
  - **Replace**: Entire existing run() method and all assertion methods
  - **Implement**:
    ```rust
    impl SpecTest for RoundRobinTest {
        fn name(&self) -> &str {
            &self.name
        }
        
        fn test_type(&self) -> TestType {
            TestType::RoundRobin
        }
        
        fn run(&self) -> bool {
            let committee_ids: Vec<OperatorId> = self.share.committee
                .iter()
                .map(|member| OperatorId::from(member.operator_id))
                .collect();
            
            for i in 0..self.heights.len() {
                let height = self.heights[i];
                let round = self.rounds[i];
                let expected_proposer = OperatorId::from(self.proposers[i]);
                
                let actual_proposer = round_robin_proposer(&committee_ids, height, round);
                
                if actual_proposer != expected_proposer {
                    eprintln!(
                        "Round robin test '{}' failed at index {}: height={}, round={}, expected={}, got={}",
                        self.name, i, height, round, expected_proposer.0, actual_proposer.0
                    );
                    return false;
                }
            }
            
            eprintln!("Round robin test '{}' passed all {} test cases", self.name, self.heights.len());
            true
        }
    }
    ```
  - **Context**: This completely replaces the consensus-based test with algorithmic validation
  - **Validation**: Test iterates through all height/round combinations and validates proposer selection
  - **Error Handling**: Clear error messages with context for debugging failures

#### Group C: Integration (Execute after Group B)
- [x] **Task #4**: Update test type enumeration and exports
  - **File**: `anchor/spec_tests/src/qbft/mod.rs`
  - **Update**: Ensure RoundRobinTest is properly exported
  - **Validate**: TestType enum includes RoundRobin variant if needed
  - **Context**: Ensure round robin tests are discoverable by test runner
  - **Integration**: Verify no breaking changes to existing test infrastructure

- [x] **Task #5**: Validate against existing Go test files
  - **Context**: Test the implementation against actual Go JSON test files
  - **Validate**: Load and run all 4 existing round robin test files:
    - `tests.RoundRobinSpecTest_qbft_round_robin_4_member_committee.json`
    - `tests.RoundRobinSpecTest_qbft_round_robin_7_member_committee.json`
    - `tests.RoundRobinSpecTest_qbft_round_robin_10_member_committee.json`
    - `tests.RoundRobinSpecTest_qbft_round_robin_13_member_committee.json`
  - **Expected**: All tests should pass with the new implementation
  - **Debug**: If any tests fail, debug algorithm or deserialization issues

#### Group D: Cleanup (Execute after Group C)
- [x] **Task #6**: Remove all unused code and imports
  - **File**: `anchor/spec_tests/src/qbft/round_robin.rs`
  - **Remove**: All QbftTestAdapter usage, ScenarioResult, ProcessingResult, ValidationResult
  - **Remove**: All message processing, error handling, and consensus-related code
  - **Clean**: Remove any unused imports or helper functions
  - **Context**: Ensure the file contains only the minimal code needed for round robin testing
  - **Validation**: Code compiles without warnings and passes all tests

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
- Tasks should be run in parallel when possible using subtasks

### Progress Tracking
The checkboxes above represent the authoritative status of each task. Keep them updated as you work.