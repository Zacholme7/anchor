# QBFT Error String Mapping Fix Implementation Plan

## Executive Summary
> **Problem Statement**: The QBFT spec tests are failing due to mismatched error strings between Rust ValidationFailure errors and expected Go error strings. Tests expect specific Go-formatted error messages like "message identifier is invalid" but receive Rust debug format strings like "MismatchedIdentifier { got: \"\", want: \"...\" }".
>
> **Proposed Solution**: Enhance the existing error mapping infrastructure to provide precise Go-compatible error string mappings for all ValidationFailure variants, with context-aware handling for specific test scenarios.
>
> **Technical Approach**: Extend the ErrorMapper in error_mapping.rs to handle complex error patterns, add context-specific mappings, and implement pattern matching for debug-formatted error strings.
>
> **Expected Outcomes**: All QBFT tests will pass with correct error string matching, improving test reliability and compatibility with Go specification requirements.

## Goals & Objectives
### Primary Goals
- Fix all identified error string mismatches in QBFT tests (6-7 failing tests out of 27 total)
- Achieve 100% QBFT test pass rate by implementing correct Go error string mappings
- Maintain backward compatibility with existing error mapping infrastructure

### Secondary Objectives
- Improve error mapping system robustness for future test additions
- Establish comprehensive coverage of all ValidationFailure variants
- Create maintainable error mapping patterns that can be extended easily

## Solution Overview
### Approach
Enhance the existing ErrorMapper system with:
1. **Pattern-based error mapping** for complex ValidationFailure formats
2. **Context-aware error handling** for test-specific scenarios
3. **Fallback detection** for unmapped error types
4. **Debug format parsing** for complex error variants

### Key Components
1. **Enhanced ErrorMapper**: Extended with pattern matching and context awareness
2. **ValidationFailure Parser**: Handles complex error formats with parameters
3. **Context-Specific Mappings**: Test-type-aware error string generation
4. **Error Pattern Registry**: Comprehensive mapping of all error patterns

### Expected Outcomes
- All 27 QBFT message tests pass with correct error string matching
- Robust error mapping system that handles future validation requirements
- Clear mapping between Rust ValidationFailure types and Go error strings
- Maintainable error handling infrastructure

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
src/qbft/adapter/
├── error_mapping.rs (Task #1: Enhanced ErrorMapper with pattern matching)
├── validation.rs (Task #2: ValidationFailure parsing utilities)
├── unified.rs (Task #3: Integration with enhanced error mapping)
│
src/qbft/
├── qbft_message.rs (Task #4: QBFT message test error handling)
├── controller_test.rs (Task #5: Controller test error handling)
├── create_message.rs (Task #6: Message creation test error handling)
├── round_robin.rs (Task #7: Round robin test error handling)
│
tests/ (Task #8: Verification and validation)
```

### Execution Plan

#### Group A: Core Error Mapping Enhancement (Execute all in parallel)
- [ ] **Task #1**: Enhanced ErrorMapper with Pattern Matching
  - **Folder**: `src/qbft/adapter/`
  - **File**: `error_mapping.rs`
  - **Imports**:
    - `use regex::Regex;`
    - `use std::collections::HashMap;`
    - `use crate::qbft::adapter::types::*;`
  - **Implements**:
    - `ErrorMapper::parse_debug_error(error_str: &str) -> Option<String>` - Parses debug format errors
    - `ErrorMapper::get_pattern_mappings() -> HashMap<String, String>` - Pattern-based error mappings
    - `ErrorMapper::map_complex_validation_failure(failure: &str, context: &TestContext) -> String` - Complex error handling
    - Enhanced `map_validation_failure()` with pattern matching support
  - **Key Patterns**:
    ```rust
    // Pattern mappings for debug format errors
    "MismatchedIdentifier" -> "message identifier is invalid"
    "UndecodableMessageData(NoMatchingVariant)" -> "message type is invalid"
    "incorrect size" -> "incorrect size"
    ```
  - **Context-Specific Mappings**:
    ```rust
    // Add to get_context_specific_mappings()
    (TestType::QbftMessage, "identifier_nil".to_string()) -> "message identifier is invalid"
    (TestType::QbftMessage, "identifier_empty".to_string()) -> "message identifier is invalid"
    (TestType::QbftMessage, "unknown_type".to_string()) -> "message type is invalid"
    ```
  - **Exports**: Enhanced `ErrorMapper` with pattern matching capabilities
  - **Integration**: Used by all QBFT test types for error string conversion

#### Group B: Validation Utilities (Execute all in parallel after Group A)
- [ ] **Task #2**: ValidationFailure Parsing Utilities
  - **Folder**: `src/qbft/adapter/`
  - **File**: `validation.rs`
  - **Imports**:
    - `use regex::Regex;`
    - `use std::str::FromStr;`
    - `use super::error_mapping::ErrorMapper;`
  - **Implements**:
    - `parse_validation_error(error_str: &str) -> ValidationErrorType` - Parses error strings
    - `detect_error_pattern(error_str: &str) -> ErrorPattern` - Detects error patterns
    - `format_go_error(error_type: ValidationErrorType, context: &TestContext) -> String` - Go formatting
  - **Error Types**:
    ```rust
    enum ValidationErrorType {
        MismatchedIdentifier { got: String, want: String },
        UnknownMessageType,
        IncorrectSize,
        NoSigners,
        DuplicatedSigner,
        Other(String),
    }
    ```
  - **Exports**: Validation error parsing utilities
  - **Integration**: Used by ErrorMapper for complex error handling

#### Group C: Test Integration (Execute all in parallel after Group B)
- [ ] **Task #3**: Enhanced Unified Adapter Integration
  - **Folder**: `src/qbft/adapter/`
  - **File**: `unified.rs`
  - **Imports**:
    - `use super::error_mapping::ErrorMapper;`
    - `use super::validation::*;`
  - **Implements**:
    - Enhanced `create_error_result()` with pattern-based error mapping
    - `map_qbft_error_to_go_string(error: &str, context: &TestContext) -> String` - QBFT error mapping
    - Updated `execute_validation_scenario()` with comprehensive error handling
  - **Key Changes**:
    ```rust
    // Enhanced error result creation
    fn create_error_result(&self, scenario_id: &str, error: AdapterError) -> ScenarioResult {
        let context = self.test_context.clone().unwrap_or_default();
        let error_mapper = ErrorMapper::new(context.clone());
        
        // Parse and map error with pattern matching
        let go_error = error_mapper.parse_and_map_error(&error.to_string());
        
        // Create comprehensive scenario result
        ScenarioResult { /* ... */ }
    }
    ```
  - **Exports**: Enhanced unified adapter with robust error mapping
  - **Integration**: Used by all QBFT test types

- [ ] **Task #4**: QBFT Message Test Error Handling
  - **Folder**: `src/qbft/`
  - **File**: `qbft_message.rs`
  - **Imports**:
    - `use super::adapter::{ErrorMapper, QbftTestAdapter, TestContext, TestType};`
  - **Implements**:
    - Enhanced `map_signed_ssv_error_to_go_string()` with comprehensive error mapping
    - `handle_validation_error(error_str: &str) -> String` - Validation error handling
    - Updated `create_signed_message()` with pattern-based error detection
  - **Key Changes**:
    ```rust
    // Enhanced error mapping for QBFT messages
    fn map_signed_ssv_error_to_go_string(&self, error: &SignedSSVMessageError) -> String {
        let error_mapper = ErrorMapper::new(TestContext::default_validation());
        match error {
            SignedSSVMessageError::NoSigners => "no signers".to_string(),
            SignedSSVMessageError::DuplicatedSigner => "non unique signer".to_string(),
            _ => error_mapper.map_ssv_error_to_go(error),
        }
    }
    ```
  - **Exports**: Enhanced QBFT message test with robust error handling
  - **Integration**: Uses ErrorMapper for consistent error string mapping

- [ ] **Task #5**: Controller Test Error Handling
  - **Folder**: `src/qbft/`
  - **File**: `controller_test.rs`
  - **Imports**:
    - `use super::adapter::{ErrorMapper, QbftTestAdapter, ScenarioResult, TestContext, TestType};`
  - **Implements**:
    - Enhanced `assert_scenario_result()` with pattern-based error matching
    - `map_controller_error(error: &str) -> String` - Controller-specific error mapping
    - Updated error handling in `run()` method
  - **Key Changes**:
    ```rust
    // Enhanced scenario result assertion
    fn assert_scenario_result(&self, result: &ScenarioResult, expected: &RunInstanceData) -> Result<(), String> {
        if !self.expected_error.is_empty() {
            let error_mapper = ErrorMapper::new(TestContext::new(self.name.clone(), TestType::Controller));
            let found_error = result.go_formatted_errors.iter()
                .any(|err| error_mapper.matches_expected_error(err, &self.expected_error));
            
            if found_error {
                return Ok(());
            } else {
                return Err(format!("Expected error '{}' not found in: {:?}", 
                    self.expected_error, result.go_formatted_errors));
            }
        }
        Ok(())
    }
    ```
  - **Exports**: Enhanced controller test with robust error handling
  - **Integration**: Uses ErrorMapper for consistent error string matching

- [ ] **Task #6**: Message Creation Test Error Handling
  - **Folder**: `src/qbft/`
  - **File**: `create_message.rs`
  - **Imports**:
    - `use super::adapter::{ErrorMapper, QbftTestAdapter, TestContext, TestType};`
  - **Implements**:
    - Enhanced `assert_message_creation_result()` with pattern-based error matching
    - `map_creation_error(error: &str) -> String` - Creation-specific error mapping
    - Updated error handling in `run()` method
  - **Key Changes**:
    ```rust
    // Enhanced message creation result assertion
    fn assert_message_creation_result(&self, result: &ScenarioResult) -> bool {
        if !self.expected_error.is_empty() {
            let error_mapper = ErrorMapper::new(TestContext::new(self.name.clone(), TestType::MessageCreation));
            return result.go_formatted_errors.iter()
                .any(|err| error_mapper.matches_expected_error(err, &self.expected_error));
        }
        // ... rest of validation logic
    }
    ```
  - **Exports**: Enhanced message creation test with robust error handling
  - **Integration**: Uses ErrorMapper for consistent error string matching

- [ ] **Task #7**: Round Robin Test Error Handling
  - **Folder**: `src/qbft/`
  - **File**: `round_robin.rs`
  - **Imports**:
    - `use super::adapter::{ErrorMapper, QbftTestAdapter, TestContext, TestType};`
  - **Implements**:
    - Enhanced `assert_round_result()` with pattern-based error matching
    - `map_round_robin_error(error: &str) -> String` - Round robin-specific error mapping
    - Updated error handling in `run()` method
  - **Key Changes**:
    ```rust
    // Enhanced round result assertion
    fn assert_round_result(&self, result: &ScenarioResult, round_index: usize) -> bool {
        if !self.expected_error.is_empty() {
            let error_mapper = ErrorMapper::new(TestContext::new(self.name.clone(), TestType::RoundRobin));
            return result.go_formatted_errors.iter()
                .any(|err| error_mapper.matches_expected_error(err, &self.expected_error));
        }
        // ... rest of validation logic
    }
    ```
  - **Exports**: Enhanced round robin test with robust error handling
  - **Integration**: Uses ErrorMapper for consistent error string matching

#### Group D: Testing and Validation (Execute after Group C)
- [ ] **Task #8**: Comprehensive Error Mapping Testing
  - **Folder**: `tests/`
  - **File**: Create comprehensive test suite
  - **Implements**:
    - `test_error_mapping_patterns()` - Tests all error pattern mappings
    - `test_context_specific_mappings()` - Tests context-aware error handling
    - `test_qbft_message_error_handling()` - Tests QBFT message error scenarios
    - `verify_all_validation_failures_mapped()` - Ensures complete coverage
  - **Test Coverage**:
    ```rust
    // Test all identified error patterns
    assert_eq!(
        error_mapper.map_validation_failure("MismatchedIdentifier"),
        "message identifier is invalid"
    );
    assert_eq!(
        error_mapper.map_validation_failure("UndecodableMessageData(NoMatchingVariant)"),
        "message type is invalid"
    );
    ```
  - **Validation**:
    - Run `cargo test --lib spec_tests::qbft_tests::test_qbft_message` to verify fixes
    - Ensure all 27 QBFT message tests pass
    - Verify no regression in other test types
  - **Exports**: Comprehensive test coverage for error mapping
  - **Integration**: Validates the entire error mapping system

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