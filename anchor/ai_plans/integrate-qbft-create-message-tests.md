# QBFT Create Message Tests Integration Implementation Plan

## Executive Summary

> **Problem**: The QBFT create message tests have been refactored with new utilities (QbftTestContext, validation facade, consolidated deserializers, error handling) but these utilities are not properly integrated into the actual test execution. The CreateMessageTest struct still uses manual parsing and duplicated logic instead of leveraging the new infrastructure.
>
> **Solution**: Fully integrate the new utilities into the CreateMessageTest implementation by refactoring the test execution flow to use QbftTestContext for setup, consolidated deserializers for JSON parsing, validation facade for comprehensive verification, and structured error handling throughout.
>
> **Technical Approach**: Replace manual committee parsing, identifier handling, and validation logic with the new utility infrastructure while maintaining full compatibility with the existing JSON test structure and expected behavior.
>
> **Expected Outcomes**: Clean, maintainable test code that leverages shared utilities, eliminates code duplication, provides comprehensive validation, and maintains 100% compatibility with existing test behavior and Go test structure.

## Goals & Objectives

### Primary Goals
- **Eliminate Code Duplication**: Remove manual committee parsing, identifier handling, and key management code in favor of shared utilities
- **Integrate New Infrastructure**: Fully utilize QbftTestContext, validation facade, consolidated deserializers, and structured error handling
- **Maintain Test Compatibility**: Preserve all existing test behavior, JSON structure compatibility, and expected outputs

### Secondary Objectives
- **Improve Error Reporting**: Use structured error types for better debugging and test failure analysis
- **Enhance Validation**: Leverage comprehensive validation facade for thorough message verification
- **Standardize Patterns**: Ensure consistent patterns across all QBFT test types

## Solution Overview

### Approach
Refactor the CreateMessageTest implementation to use the new utility infrastructure while maintaining the existing SpecTest trait interface and JSON compatibility. Replace manual parsing and setup logic with builder patterns and shared utilities.

### Key Components
1. **CreateMessageTest Refactoring**: Update to use QbftTestContext and CommonTestFields
2. **Deserializer Consolidation**: Remove duplicated deserializers and use utils/deserializers.rs
3. **Validation Integration**: Replace manual validation with QbftValidationFacade
4. **Error Handling**: Use structured QbftSpecTestError throughout
5. **Test Flow Enhancement**: Streamline the test execution flow with new utilities

### Architecture Diagram
```
JSON Test File → CommonTestFields Parsing → QbftTestContext Builder → UnifiedTestAdapter → Message Creation → Validation Facade → Results
```

### Data Flow
```
JSON Input → Consolidated Deserializers → QbftTestContext → UnifiedTestAdapter Setup → QBFT Message Creation → Comprehensive Validation → Test Results
```

### Expected Outcomes
- CreateMessageTest uses shared utilities instead of manual parsing
- All existing tests continue to pass with identical behavior
- Enhanced error reporting with structured error types
- Comprehensive validation using validation facade
- Eliminated code duplication and improved maintainability

## Implementation Tasks

### CRITICAL IMPLEMENTATION RULES
1. **MAINTAIN COMPATIBILITY**: All existing tests must continue to pass with identical behavior
2. **NO BREAKING CHANGES**: Preserve the SpecTest trait interface and JSON structure support
3. **COMPREHENSIVE INTEGRATION**: Use all new utilities - QbftTestContext, validation facade, consolidated deserializers, error handling
4. **PRODUCTION READY**: No placeholders, todos, or incomplete implementations
5. **PRESERVE BEHAVIOR**: Maintain exact same test execution flow and expected outcomes

### Visual Dependency Tree
```
anchor/spec_tests/src/qbft/
├── create_message.rs (Task #0: Refactor CreateMessageTest implementation)
├── mod.rs (Task #1: Remove duplicate deserializers)
├── common_types.rs (Task #2: Ensure CommitteeMember compatibility)
├── test_utils.rs (Task #3: Add create message specific utilities)
├── validation_facade.rs (Task #4: Add message creation validation)
└── error.rs (Task #5: Add create message error handling)
```

### Execution Plan

#### Group A: Foundation Updates (Execute in parallel)
- [ ] **Task #0**: Remove duplicate CommitteeMember and Operator types
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `create_message.rs`
  - **Remove**: Lines 66-95 (CommitteeMember and Operator struct definitions)
  - **Replace with**: Import from common_types
  - **Imports to add**:
    ```rust
    use super::common_types::{SpecTestCommitteeMember, SpecTestOperator};
    use super::test_utils::{QbftTestContext, json_parsing::CommonTestFields};
    use super::validation_facade::QbftValidationFacade;
    use super::error::{QbftSpecTestError, QbftSpecTestResult};
    ```
  - **Context**: Eliminates duplicate type definitions and prepares for utility integration

- [ ] **Task #1**: Consolidate deserializers in mod.rs
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `mod.rs`
  - **Remove**: Lines 42-118 (duplicate qbft_deserializers module)
  - **Replace with**: Re-export from utils/deserializers.rs
  - **Update imports**:
    ```rust
    pub use crate::utils::deserializers::qbft_deserializers::*;
    ```
  - **Context**: Removes duplicate deserializers and uses consolidated ones from utils

#### Group B: CreateMessageTest Structure Updates (Execute after Group A)
- [ ] **Task #2**: Update CreateMessageTest struct to use shared types
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `create_message.rs`
  - **Update struct definition**:
    ```rust
    #[derive(Deserialize)]
    pub struct CreateMessageTest {
        // Common fields that can be extracted
        #[serde(flatten)]
        pub common_fields: CommonTestFields,
        
        // QBFT-specific fields
        #[serde(rename = "Value", deserialize_with = "deserialize_value_into_root")]
        pub root: Hash256,
        
        #[serde(rename = "StateValue")]
        pub state_value: Option<String>,
        
        #[serde(rename = "Round", deserialize_with = "deserialize_u64_into_round")]
        pub round: Option<Round>,
        
        #[serde(rename = "RoundChangeJustifications")]
        pub round_change_justifications: Option<Vec<SignedSSVMessage>>,
        
        #[serde(rename = "PrepareJustifications")]
        pub prepare_justifications: Option<Vec<SignedSSVMessage>>,
        
        #[serde(rename = "CreateType", deserialize_with = "deserialize_qbft_message_type")]
        pub create_type: QbftMessageType,
        
        #[serde(rename = "ExpectedRoot")]
        pub expected_root: Hash256,
        
        // Test execution state (not deserialized)
        #[serde(skip)]
        pub unified_adapter: Option<UnifiedTestAdapter>,
        
        #[serde(skip)]
        pub test_context: Option<QbftTestContext>,
    }
    ```
  - **Context**: Uses CommonTestFields for shared data and structured organization

#### Group C: Core Implementation Updates (Execute after Group B)
- [ ] **Task #3**: Refactor create_qbft_instance to use QbftTestContext
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `create_message.rs`
  - **Replace create_qbft_instance method**:
    ```rust
    fn create_qbft_instance(&self) -> QbftSpecTestResult<UnifiedTestAdapter> {
        // Build test context from common fields and specific data
        let mut context_builder = QbftTestContext::builder()
            .with_name(self.common_fields.name.clone())
            .with_type(self.common_fields.test_type.clone())
            .with_documentation(self.common_fields.documentation.clone());
        
        // Add committee member if present
        if let Some(committee_member) = &self.common_fields.committee_member {
            context_builder = context_builder.with_committee_member(committee_member.clone());
        }
        
        // Add identifier if present
        if let Some(identifier) = &self.common_fields.identifier {
            context_builder = context_builder.with_identifier(identifier.clone());
        }
        
        // Add operator ID if present
        if let Some(operator_id) = self.common_fields.operator_id {
            context_builder = context_builder.with_operator_id(operator_id);
        }
        
        // Build context and create adapter
        let context = context_builder.build()?;
        let adapter = context.create_unified_adapter()?;
        
        Ok(adapter)
    }
    ```
  - **Context**: Replaces manual parsing with QbftTestContext builder pattern

- [ ] **Task #4**: Refactor setup_test_scenario to use enhanced state management
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `create_message.rs`
  - **Replace setup_test_scenario method**:
    ```rust
    fn setup_test_scenario(&self, unified_adapter: &mut UnifiedTestAdapter) -> QbftSpecTestResult<()> {
        // Extract prepared state from test data
        let (last_prepared_round, last_prepared_value) = self.extract_prepared_state()?;
        
        // Create test scenario using utilities
        let scenario = TestScenario {
            round: self.round.unwrap_or(1.into()),
            last_prepared_round,
            last_prepared_value,
            round_change_justifications: self.round_change_justifications.clone().unwrap_or_default(),
            prepare_justifications: self.prepare_justifications.clone().unwrap_or_default(),
        };
        
        // Configure the adapter with the scenario
        unified_adapter.set_test_scenario(scenario)?;
        
        Ok(())
    }
    ```
  - **Context**: Uses structured scenario setup with enhanced error handling

- [ ] **Task #5**: Refactor create_and_verify_message to use validation facade
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `create_message.rs`
  - **Replace create_and_verify_message method**:
    ```rust
    fn create_and_verify_message(&self, unified_adapter: &mut UnifiedTestAdapter) -> QbftSpecTestResult<bool> {
        // Handle expected error cases
        if self.common_fields.expects_error() {
            return self.verify_expected_error(unified_adapter);
        }
        
        // Create the message using the unified adapter
        let signed_message = unified_adapter.create_message(
            self.create_type,
            self.root,
            self.round,
            self.round_change_justifications.clone(),
            self.prepare_justifications.clone(),
        )?;
        
        // Use validation facade for comprehensive verification
        let validation_facade = QbftValidationFacade::with_committee(unified_adapter.get_committee())?;
        
        // Validate the message comprehensively
        let validation_result = validation_facade.validate_comprehensive(
            &signed_message,
            Some(self.expected_root)
        )?;
        
        if !validation_result {
            return Err(QbftSpecTestError::test_execution(
                format!("Message validation failed. Expected root: {}, Actual root: {}",
                    self.expected_root,
                    validation_facade.get_message_root(&signed_message)
                )
            ));
        }
        
        Ok(true)
    }
    ```
  - **Context**: Uses validation facade for comprehensive message verification

- [ ] **Task #6**: Add expected error handling support
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `create_message.rs`
  - **Add new method**:
    ```rust
    fn verify_expected_error(&self, unified_adapter: &mut UnifiedTestAdapter) -> QbftSpecTestResult<bool> {
        // Attempt to create the message (should fail)
        let result = unified_adapter.create_message(
            self.create_type,
            self.root,
            self.round,
            self.round_change_justifications.clone(),
            self.prepare_justifications.clone(),
        );
        
        match result {
            Err(error) => {
                // Check if the error message matches expected
                let expected_error = self.common_fields.expected_error_message();
                let actual_error = error.to_string();
                
                if actual_error.contains(expected_error) {
                    Ok(true)
                } else {
                    Err(QbftSpecTestError::test_execution(
                        format!("Expected error '{}' but got '{}'", expected_error, actual_error)
                    ))
                }
            }
            Ok(_) => {
                Err(QbftSpecTestError::test_execution(
                    format!("Expected error '{}' but message creation succeeded", 
                           self.common_fields.expected_error_message())
                ))
            }
        }
    }
    ```
  - **Context**: Handles negative test cases where errors are expected

#### Group D: Integration and Cleanup (Execute after Group C)
- [ ] **Task #7**: Update SpecTest trait implementation
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `create_message.rs`
  - **Update SpecTest implementation**:
    ```rust
    impl SpecTest for CreateMessageTest {
        fn name(&self) -> &str {
            &self.common_fields.name
        }
        
        fn run(&self) -> bool {
            match self.run_test_with_structured_errors() {
                Ok(success) => success,
                Err(error) => {
                    eprintln!("Test '{}' failed: {}", self.name(), error);
                    false
                }
            }
        }
        
        fn setup(&mut self) {
            // Setup is handled in run() method
        }
        
        fn test_type() -> SpecTestType {
            SpecTestType::Qbft(QbftSpecTestType::CreateMessage)
        }
    }
    ```
  - **Context**: Updates to use common fields and structured error handling

- [ ] **Task #8**: Add comprehensive test execution method
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `create_message.rs`
  - **Add new method**:
    ```rust
    fn run_test_with_structured_errors(&self) -> QbftSpecTestResult<bool> {
        // Validate test data
        self.validate_test_data()?;
        
        // Create QBFT instance using utilities
        let mut unified_adapter = self.create_qbft_instance()?;
        
        // Setup test scenario
        self.setup_test_scenario(&mut unified_adapter)?;
        
        // Create and verify message
        self.create_and_verify_message(&mut unified_adapter)
    }
    
    fn validate_test_data(&self) -> QbftSpecTestResult<()> {
        // Validate that required fields are present
        if self.common_fields.committee_member.is_none() {
            return Err(QbftSpecTestError::scenario_setup(
                "CommitteeMember is required for create message tests"
            ));
        }
        
        if self.common_fields.identifier.is_none() {
            return Err(QbftSpecTestError::scenario_setup(
                "Identifier is required for create message tests"
            ));
        }
        
        if self.common_fields.operator_id.is_none() {
            return Err(QbftSpecTestError::scenario_setup(
                "OperatorID is required for create message tests"
            ));
        }
        
        // Validate justifications format if present
        if let Some(justifications) = &self.round_change_justifications {
            if justifications.is_empty() {
                return Err(QbftSpecTestError::scenario_setup(
                    "RoundChangeJustifications cannot be empty if present"
                ));
            }
        }
        
        if let Some(justifications) = &self.prepare_justifications {
            if justifications.is_empty() {
                return Err(QbftSpecTestError::scenario_setup(
                    "PrepareJustifications cannot be empty if present"
                ));
            }
        }
        
        Ok(())
    }
    ```
  - **Context**: Provides structured error handling and comprehensive validation

- [ ] **Task #9**: Remove deprecated methods and cleanup
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `create_message.rs`
  - **Remove deprecated methods**:
    - `validate_justification_data()` - replaced by `validate_test_data()`
    - `parse_message_id_from_base64()` - now in test_utils
    - `parse_rsa_key_from_base64()` - now in test_utils
    - `extract_prepared_state()` - enhanced version in new flow
  - **Update imports**: Remove unused imports and add new ones
  - **Context**: Cleans up deprecated code and unused imports

#### Group E: Testing and Validation (Execute after Group D)
- [ ] **Task #10**: Add integration tests for new functionality
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `create_message.rs`
  - **Add test module**:
    ```rust
    #[cfg(test)]
    mod tests {
        use super::*;
        use serde_json;
        
        #[test]
        fn test_create_message_with_new_utilities() {
            let json_data = r#"
            {
                "Name": "test create proposal",
                "Type": "Message creation test",
                "Documentation": "Test message creation",
                "Value": [1,2,3,4,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
                "Round": 1,
                "CreateType": "createProposal",
                "ExpectedRoot": "43c23219aaf744537a2e8b3896937a2e9aa24a8eaf8fcabf8ec7376f76669f3c",
                "ExpectedError": "",
                "CommitteeMember": {
                    "OperatorID": 1,
                    "Committee": [{"OperatorID": 1, "SSVOperatorPubKey": "test"}]
                },
                "Identifier": "AQIDBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                "OperatorID": 1
            }
            "#;
            
            let test: CreateMessageTest = serde_json::from_str(json_data).unwrap();
            assert_eq!(test.common_fields.name, "test create proposal");
            assert!(!test.common_fields.expects_error());
            
            // Test that the new utilities work
            let result = test.run_test_with_structured_errors();
            assert!(result.is_ok());
        }
        
        #[test]
        fn test_expected_error_handling() {
            let json_data = r#"
            {
                "Name": "test expected error",
                "Type": "Error test",
                "Documentation": "Test error handling",
                "Value": [1,2,3,4,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
                "Round": 1,
                "CreateType": "createProposal",
                "ExpectedRoot": "0000000000000000000000000000000000000000000000000000000000000000",
                "ExpectedError": "invalid state",
                "CommitteeMember": {
                    "OperatorID": 1,
                    "Committee": [{"OperatorID": 1, "SSVOperatorPubKey": "test"}]
                },
                "Identifier": "AQIDBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                "OperatorID": 1
            }
            "#;
            
            let test: CreateMessageTest = serde_json::from_str(json_data).unwrap();
            assert!(test.common_fields.expects_error());
            assert_eq!(test.common_fields.expected_error_message(), "invalid state");
        }
    }
    ```
  - **Context**: Ensures new functionality works correctly

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