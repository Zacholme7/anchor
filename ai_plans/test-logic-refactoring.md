# Test Logic Refactoring Implementation Plan

## Executive Summary

> **Problem Statement**: Test files contain significant business logic that belongs in the adapter layer, including complex error mapping (270+ lines), message processing orchestration, QBFT validation, and scenario setup. Additionally, custom validation logic duplicates functionality already available in the message_validator crate.

> **Proposed Solution**: Refactor test files to contain only test orchestration logic while moving all business logic (QBFT processing, message validation, error mapping, scenario setup) into the adapter directory. Replace custom validation with message_validator functions and centralize ValidationFailure → Go error string mapping.

> **Technical Approach**: Create new adapter modules for error mapping, scenario management, and enhanced validation. Move ~500 lines of business logic from test files to adapters, ensuring tests focus purely on data loading, method calls, and result assertions.

> **Expected Outcomes**: Clean separation of concerns, reusable business logic, proper utilization of message_validator, and maintainable test files that focus on verification rather than implementation.

## Goals & Objectives

### Primary Goals
- **Move Business Logic to Adapters**: Extract ~500 lines of QBFT logic, error mapping, and validation from test files to adapter modules
- **Utilize message_validator Properly**: Replace custom validation with existing ValidationFailure types and validation functions

### Secondary Objectives
- **Centralized Error Mapping**: Single source of truth for ValidationFailure → Go error string conversion
- **Clean Test Separation**: Tests only orchestrate, adapters handle all business logic
- **Reusable Validation Infrastructure**: Consistent validation patterns across all test scenarios

## Solution Overview

### Approach
Extract business logic from test files into specialized adapter modules, replacing custom validation with message_validator functions, and establishing clean interfaces between tests and business logic.

### Key Components
1. **Error Mapping Module**: Centralized ValidationFailure → Go error string conversion
2. **Enhanced Validation Module**: Full utilization of message_validator functions  
3. **Scenario Management Module**: Encapsulated test scenario setup and configuration
4. **Enhanced Message Processor**: Complete message processing implementation
5. **Clean Test Files**: Pure orchestration without business logic

### Architecture Diagram
```
Tests → AdapterBuilder → QbftTestAdapter → [Validation|Processing|Scenarios|ErrorMapping]
                                                ↓
                                        message_validator
                                                ↓
                                        ValidationFailure → Go Error Strings
```

### Data Flow
```
Test Data → Adapter Setup → Business Logic Processing → ValidationFailure → Go Error Mapping → Test Assertions
```

### Expected Outcomes
- **Tests contain only orchestration logic** (data loading, method calls, assertions)
- **All QBFT logic resides in adapters** with proper separation of concerns
- **ValidationFailure types are used consistently** instead of string parsing
- **Error mapping is centralized and reusable** across all test types

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
anchor/spec_tests/src/qbft/
├── adapter/
│   ├── mod.rs (Task #4: Clean exports for enhanced framework)
│   ├── base.rs (Task #2: Enhanced QbftTestAdapter with business logic)
│   ├── builder.rs (Task #2: Enhanced builder with error handling)
│   ├── types.rs (Task #0: Enhanced types with error contexts)
│   ├── validation.rs (Task #1: Complete message_validator integration)
│   ├── error_mapping.rs (Task #1: Centralized Go error string mapping)
│   ├── scenario.rs (Task #1: Test scenario setup encapsulation)
│   └── processor.rs (Task #2: Complete message processing implementation)
│
├── controller_test.rs (Task #3: Clean test orchestration only)
├── create_message.rs (Task #3: Clean test orchestration only)
├── qbft_message.rs (Task #3: Clean test orchestration only)
└── round_robin.rs (Task #3: Clean test orchestration only)
```

### Execution Plan

#### Group A: Foundation Enhancement (Execute all in parallel)
- [x] **Task #0**: Enhance types with error context support
  - Folder: `anchor/spec_tests/src/qbft/adapter/`
  - File: `types.rs`
  - Imports:
    - `use message_validator::ValidationFailure`
    - `use ssv_types::{Round, OperatorId, message::SignedSSVMessage}`
    - `use std::collections::HashMap`
  - Implements:
    ```rust
    #[derive(Debug, Clone)]
    pub struct ValidationResult {
        pub is_valid: bool,
        pub errors: Vec<ValidationFailure>,
        pub warnings: Vec<String>,
    }
    
    #[derive(Debug, Clone)]
    pub struct ProcessingResult {
        pub consensus_reached: bool,
        pub messages_sent: Vec<SignedSSVMessage>,
        pub validation_result: ValidationResult,
        pub go_error_messages: Vec<String>, // Pre-mapped for test assertions
    }
    
    #[derive(Debug, Clone)]
    pub struct TestContext {
        pub test_name: String,
        pub test_type: TestType,
        pub expected_errors: Vec<String>,
        pub error_mapping_context: HashMap<String, String>,
    }
    
    #[derive(Debug, Clone)]
    pub enum TestType {
        Controller,
        MessageCreation,
        QbftMessage,
        RoundRobin,
    }
    
    #[derive(Debug, Clone)]
    pub struct ScenarioResult {
        pub scenario_id: String,
        pub processing_result: ProcessingResult,
        pub decided_state: DecidedState,
        pub timer_state: Option<TimerState>,
        pub validation_errors: Vec<ValidationFailure>,
        pub go_formatted_errors: Vec<String>,
    }
    ```
  - Exports: All enhanced types with validation support
  - Context: Foundation types that support proper error handling and test context

- [x] **Task #1**: Create centralized error mapping module
  - Folder: `anchor/spec_tests/src/qbft/adapter/`
  - File: `error_mapping.rs`
  - Imports:
    - `use message_validator::ValidationFailure`
    - `use super::types::{TestType, TestContext}`
    - `use std::collections::HashMap`
  - Implements:
    ```rust
    pub struct ErrorMapper {
        context: TestContext,
        validation_mappings: HashMap<ValidationFailure, String>,
        context_specific_mappings: HashMap<(TestType, String), String>,
    }
    
    impl ErrorMapper {
        pub fn new(context: TestContext) -> Self;
        
        pub fn map_validation_failure(&self, failure: &ValidationFailure) -> String;
        pub fn map_validation_failures(&self, failures: &[ValidationFailure]) -> Vec<String>;
        
        pub fn map_qbft_error(&self, error: &str) -> String;
        pub fn map_adapter_error(&self, error: &super::AdapterError) -> String;
        
        // Context-aware mapping based on test type and name
        pub fn map_with_context(&self, failure: &ValidationFailure, test_name: &str) -> String;
        
        // Batch mapping for scenario results
        pub fn map_scenario_errors(&self, failures: &[ValidationFailure]) -> Vec<String>;
        
        // Standard Go error format mappings
        fn get_base_mappings() -> HashMap<ValidationFailure, String>;
        fn get_controller_specific_mappings() -> HashMap<String, String>;
        fn get_message_creation_specific_mappings() -> HashMap<String, String>;
    }
    
    // Direct mapping functions for common scenarios
    pub fn map_validation_failure_to_go_format(failure: &ValidationFailure) -> String;
    pub fn map_validation_failures_to_go_format(failures: &[ValidationFailure]) -> Vec<String>;
    
    // Context-aware mapping function
    pub fn map_with_test_context(
        failure: &ValidationFailure,
        test_type: TestType,
        test_name: &str,
    ) -> String;
    ```
  - Validation Mappings Implementation:
    ```rust
    // Complete mapping of all ValidationFailure variants to Go error strings
    ValidationFailure::NoSigners => "no signers",
    ValidationFailure::DuplicatedSigner => "duplicated signer", 
    ValidationFailure::SignerNotInCommittee => "signer not in committee",
    ValidationFailure::ZeroRound => "zero round",
    ValidationFailure::SignersNotSorted => "signers not sorted",
    ValidationFailure::EmptyData => "empty data",
    ValidationFailure::SignersAndSignaturesWithDifferentLength => "signers and signatures with different length",
    ValidationFailure::NonDecidedWithMultipleSigners { got, want } => format!("non decided with multiple signers: got {}, want {}", got, want),
    ValidationFailure::DecidedNotEnoughSigners { got, want } => format!("decided not enough signers: got {}, want {}", got, want),
    ValidationFailure::SignatureVerificationFailed { reason } => format!("msg signature invalid: {}", reason),
    // ... all 50+ ValidationFailure variants mapped to exact Go test expectations
    ```
  - Exports: ErrorMapper, mapping functions
  - Context: Centralized error mapping logic moved from test files

- [x] **Task #1**: Enhance validation module with message_validator integration
  - Folder: `anchor/spec_tests/src/qbft/adapter/`
  - File: `validation.rs`
  - Imports:
    - `use message_validator::{ValidationFailure, validate_consensus_message, minimal_validate_consensus_message}`
    - `use ssv_types::{message::SignedSSVMessage, IndexSet, OperatorId, consensus::QbftMessageType}`
    - `use super::types::{ValidationResult, TestContext}`
    - `use super::error_mapping::ErrorMapper`
  - Implements:
    ```rust
    // Replace custom validation with message_validator functions
    pub fn validate_message_comprehensive(
        msg: &SignedSSVMessage,
        committee: &IndexSet<OperatorId>,
        context: &TestContext,
    ) -> ValidationResult {
        // Use message_validator's comprehensive validation
        let mut errors = Vec::new();
        
        // Basic message validation using message_validator
        if let Err(failure) = minimal_validate_consensus_message(msg) {
            errors.push(failure);
        }
        
        // Committee-specific validation
        if let Err(failure) = validate_message_committee(msg, committee) {
            errors.push(failure);
        }
        
        // QBFT-specific validation based on message type
        if let Err(failures) = validate_qbft_message_type(msg) {
            errors.extend(failures);
        }
        
        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings: Vec::new(),
        }
    }
    
    pub fn validate_message_basic_enhanced(msg: &SignedSSVMessage) -> Result<(), ValidationFailure> {
        // Use message_validator instead of custom logic
        minimal_validate_consensus_message(msg)
    }
    
    pub fn validate_justifications_enhanced(
        justifications: &[SignedSSVMessage],
        committee: &IndexSet<OperatorId>,
    ) -> ValidationResult {
        let mut all_errors = Vec::new();
        
        for (i, justification) in justifications.iter().enumerate() {
            if let Err(failure) = minimal_validate_consensus_message(justification) {
                all_errors.push(failure);
            }
        }
        
        ValidationResult {
            is_valid: all_errors.is_empty(),
            errors: all_errors,
            warnings: Vec::new(),
        }
    }
    
    // QBFT-specific validation using message_validator patterns
    pub fn validate_qbft_message_type(msg: &SignedSSVMessage) -> Result<(), Vec<ValidationFailure>> {
        // Replace custom qbft_message.rs validation with message_validator functions
        // Validate based on QbftMessageType (Proposal, Prepare, Commit, RoundChange)
        let message_type = extract_qbft_message_type(msg)?;
        
        match message_type {
            QbftMessageType::Proposal => validate_proposal_message(msg),
            QbftMessageType::Prepare => validate_prepare_message(msg),
            QbftMessageType::Commit => validate_commit_message(msg),
            QbftMessageType::RoundChange => validate_round_change_message(msg),
        }
    }
    
    // Specialized validation functions using message_validator
    fn validate_proposal_message(msg: &SignedSSVMessage) -> Result<(), Vec<ValidationFailure>>;
    fn validate_prepare_message(msg: &SignedSSVMessage) -> Result<(), Vec<ValidationFailure>>;
    fn validate_commit_message(msg: &SignedSSVMessage) -> Result<(), Vec<ValidationFailure>>;
    fn validate_round_change_message(msg: &SignedSSVMessage) -> Result<(), Vec<ValidationFailure>>;
    
    // Enhanced validation with Go error formatting
    pub fn validate_and_format_errors(
        msg: &SignedSSVMessage,
        committee: &IndexSet<OperatorId>,
        context: &TestContext,
    ) -> (ValidationResult, Vec<String>) {
        let validation_result = validate_message_comprehensive(msg, committee, context);
        let error_mapper = ErrorMapper::new(context.clone());
        let go_formatted_errors = error_mapper.map_validation_failures(&validation_result.errors);
        
        (validation_result, go_formatted_errors)
    }
    ```
  - Exports: Enhanced validation functions using message_validator
  - Context: Complete replacement of custom validation with message_validator integration

- [x] **Task #1**: Create scenario management module
  - Folder: `anchor/spec_tests/src/qbft/adapter/`
  - File: `scenario.rs`
  - Imports:
    - `use super::types::{ScenarioConfig, TestContext, ScenarioResult, AdapterState}`
    - `use super::base::QbftTestAdapter`
    - `use super::validation::validate_and_format_errors`
    - `use ssv_types::{Round, message::SignedSSVMessage}`
    - `use types::Hash256`
    - `use base64::Engine`
    - `use sha2::{Digest, Sha256}`
  - Implements:
    ```rust
    pub struct ScenarioManager {
        context: TestContext,
        adapter: QbftTestAdapter,
        current_scenario: Option<String>,
    }
    
    impl ScenarioManager {
        pub fn new(adapter: QbftTestAdapter, context: TestContext) -> Self;
        
        // Scenario setup (moved from create_message.rs)
        pub fn setup_message_creation_scenario(
            &mut self,
            round: Option<Round>,
            state_value: Option<String>,
            round_change_justifications: Vec<SignedSSVMessage>,
            prepare_justifications: Vec<SignedSSVMessage>,
        ) -> Result<(), AdapterError>;
        
        // Controller scenario setup (moved from controller_test.rs)
        pub fn setup_controller_scenario(
            &mut self,
            scenario_id: String,
            input_value: Option<String>,
            height: Option<u64>,
        ) -> Result<(), AdapterError>;
        
        // Message validation scenario (moved from qbft_message.rs) 
        pub fn setup_validation_scenario(
            &mut self,
            message: SignedSSVMessage,
            expected_root: Option<Hash256>,
        ) -> Result<(), AdapterError>;
        
        // Execute scenario with complete error handling
        pub fn execute_scenario(
            &mut self,
            input_messages: Vec<SignedSSVMessage>,
        ) -> ScenarioResult;
        
        // Scenario state management
        pub fn reset_scenario(&mut self);
        pub fn get_scenario_state(&self) -> &AdapterState;
        
        // Helper methods (moved from test files)
        fn extract_prepared_state(&self, state_value: &str) -> Result<Option<(Round, Hash256)>, AdapterError>;
        fn decode_input_value(&self, input_value_b64: &str) -> Result<Vec<u8>, AdapterError>;
        fn validate_scenario_expectations(&self, result: &ScenarioResult) -> Vec<String>;
    }
    
    // Scenario-specific result processing
    pub fn process_controller_scenario_result(
        adapter: &QbftTestAdapter,
        context: &TestContext,
    ) -> ScenarioResult;
    
    pub fn process_message_creation_result(
        message: SignedSSVMessage,
        expected_root: Hash256,
        context: &TestContext,
    ) -> ScenarioResult;
    
    pub fn process_validation_result(
        validation_failures: Vec<ValidationFailure>,
        context: &TestContext,
    ) -> ScenarioResult;
    ```
  - Exports: ScenarioManager and result processing functions
  - Context: Encapsulates scenario setup logic moved from test files

#### Group B: Enhanced Business Logic (Execute all in parallel after Group A)
- [x] **Task #2**: Enhance QbftTestAdapter with complete business logic
  - Folder: `anchor/spec_tests/src/qbft/adapter/`
  - File: `base.rs`
  - Additional Imports:
    - `use super::error_mapping::{ErrorMapper, map_validation_failure_to_go_format}`
    - `use super::validation::{validate_message_comprehensive, validate_and_format_errors}`
    - `use super::scenario::{ScenarioManager, process_controller_scenario_result}`
  - Enhanced Implementation:
    ```rust
    impl QbftTestAdapter {
        // Enhanced message processing with complete error handling
        pub fn process_messages_with_context(
            &mut self,
            messages: Vec<SignedSSVMessage>,
            context: &TestContext,
        ) -> ScenarioResult {
            let mut all_validation_errors = Vec::new();
            let mut processed_messages = Vec::new();
            
            // Process each message with comprehensive validation
            for message in messages {
                let (validation_result, go_errors) = validate_and_format_errors(
                    &message, 
                    &self.committee, 
                    context
                );
                
                if validation_result.is_valid {
                    processed_messages.push(message);
                } else {
                    all_validation_errors.extend(validation_result.errors);
                }
            }
            
            // Process valid messages through QBFT
            let processing_result = self.process_valid_messages(processed_messages, context)?;
            
            // Update consensus state
            self.update_consensus_state();
            
            // Create comprehensive scenario result
            process_controller_scenario_result(self, context)
        }
        
        // Enhanced message creation with validation
        pub fn create_message_with_validation(
            &mut self,
            request: MessageCreationRequest,
            context: &TestContext,
        ) -> Result<(SignedSSVMessage, ValidationResult), AdapterError> {
            // Create message using QBFT
            let message = self.create_message(request)?;
            
            // Validate created message
            let (validation_result, _) = validate_and_format_errors(
                &message,
                &self.committee,
                context,
            );
            
            Ok((message, validation_result))
        }
        
        // Enhanced error reporting
        pub fn get_validation_errors_formatted(&self, context: &TestContext) -> Vec<String> {
            let error_mapper = ErrorMapper::new(context.clone());
            // Map current validation state to Go format strings
            error_mapper.map_scenario_errors(&self.get_current_validation_errors())
        }
        
        // Enhanced state inspection with context
        pub fn get_scenario_result(&self, context: &TestContext) -> ScenarioResult {
            process_controller_scenario_result(self, context)
        }
        
        // Business logic methods (moved from test files)
        pub fn execute_controller_test_scenario(
            &mut self,
            input_value: Option<Vec<u8>>,
            input_messages: Vec<SignedSSVMessage>,
            context: &TestContext,
        ) -> ScenarioResult;
        
        pub fn execute_message_creation_scenario(
            &mut self,
            request: MessageCreationRequest,
            expected_root: Hash256,
            context: &TestContext,
        ) -> ScenarioResult;
        
        pub fn execute_validation_scenario(
            &mut self,
            message: SignedSSVMessage,
            context: &TestContext,
        ) -> ScenarioResult;
        
        // Helper methods (moved from tests)
        fn validate_timer_state_expectations(
            &self,
            expected: &ExpectedTimerState,
            context: &TestContext,
        ) -> Vec<String>;
        
        fn validate_decided_state_expectations(
            &self,
            expected: &ExpectedDecidedState,
            context: &TestContext,
        ) -> Vec<String>;
    }
    ```
  - Exports: Enhanced QbftTestAdapter with complete business logic
  - Context: All business logic moved from tests into adapter with proper error handling

- [ ] **Task #2**: Enhance message processor with complete implementation
  - Folder: `anchor/spec_tests/src/qbft/adapter/`
  - File: `processor.rs`
  - Enhanced Implementation:
    ```rust
    impl MessageProcessor {
        // Complete message processing implementation
        pub fn process_message_batch_comprehensive(
            &mut self,
            messages: Vec<SignedSSVMessage>,
            qbft: &mut qbft::Qbft<DefaultLeaderFunction, BeaconVote, SharedMessageSender>,
            context: &TestContext,
        ) -> ScenarioResult {
            let mut validation_results = Vec::new();
            let mut processed_messages = Vec::new();
            let error_mapper = ErrorMapper::new(context.clone());
            
            // Comprehensive validation and processing
            for message in messages {
                let validation_result = validate_message_comprehensive(
                    &message,
                    context.committee(),
                    context,
                );
                
                if validation_result.is_valid {
                    processed_messages.push(message);
                } else {
                    validation_results.extend(validation_result.errors);
                }
            }
            
            // Process through QBFT with complete state tracking
            let consensus_reached = self.process_through_qbft(processed_messages, qbft)?;
            
            // Create comprehensive result
            ScenarioResult {
                scenario_id: context.scenario_id(),
                processing_result: ProcessingResult {
                    consensus_reached,
                    messages_sent: self.extract_outgoing_messages_enhanced(&message_queue),
                    validation_result: ValidationResult {
                        is_valid: validation_results.is_empty(),
                        errors: validation_results.clone(),
                        warnings: Vec::new(),
                    },
                    go_error_messages: error_mapper.map_validation_failures(&validation_results),
                },
                decided_state: self.get_decided_state(),
                timer_state: self.get_timer_state(),
                validation_errors: validation_results,
                go_formatted_errors: error_mapper.map_validation_failures(&validation_results),
            }
        }
        
        // Enhanced QBFT integration
        fn process_through_qbft(
            &mut self,
            messages: Vec<SignedSSVMessage>,
            qbft: &mut qbft::Qbft<DefaultLeaderFunction, BeaconVote, SharedMessageSender>,
        ) -> Result<bool, AdapterError>;
        
        // Enhanced message conversion and signing
        fn extract_outgoing_messages_enhanced(
            &self,
            message_queue: &Arc<RwLock<VecDeque<UnsignedWrappedQbftMessage>>>,
        ) -> Vec<SignedSSVMessage>;
    }
    ```
  - Exports: Enhanced MessageProcessor with complete business logic
  - Context: Complete message processing implementation using message_validator

- [ ] **Task #2**: Enhance builder with error handling integration
  - Folder: `anchor/spec_tests/src/qbft/adapter/`
  - File: `builder.rs`
  - Enhanced Implementation:
    ```rust
    impl AdapterBuilder {
        // Enhanced builder methods with context support
        pub fn with_test_context(mut self, context: TestContext) -> Self;
        
        pub fn for_controller_testing_with_context(
            committee_member: &SpecTestCommitteeMember,
            operator_id: OperatorId,
            instance_height: u64,
            current_height: u64,
            test_context: TestContext,
        ) -> Result<(QbftTestAdapter, ScenarioManager), AdapterError> {
            let adapter = Self::for_controller_testing(
                committee_member,
                operator_id, 
                instance_height,
                current_height,
            )?;
            
            let scenario_manager = ScenarioManager::new(adapter, test_context);
            Ok((scenario_manager.adapter, scenario_manager))
        }
        
        pub fn for_message_creation_with_context(
            committee_member: &SpecTestCommitteeMember,
            operator_id: OperatorId,
            test_context: TestContext,
        ) -> Result<(QbftTestAdapter, ScenarioManager), AdapterError>;
        
        pub fn for_validation_testing_with_context(
            committee_member: &SpecTestCommitteeMember,
            operator_id: OperatorId,
            test_context: TestContext,
        ) -> Result<(QbftTestAdapter, ScenarioManager), AdapterError>;
    }
    ```
  - Exports: Enhanced builder with context and scenario support
  - Context: Builder patterns that create properly configured business logic components

#### Group C: Test File Refactoring (Execute all in parallel after Group B)
- [ ] **Task #3**: Refactor all test files to pure orchestration
  - Files: `controller_test.rs`, `create_message.rs`, `qbft_message.rs`, `round_robin.rs`
  - **controller_test.rs Complete Refactoring**:
    ```rust
    // Remove ALL business logic methods (270+ lines):
    // - map_error_to_go_format() → moved to adapter/error_mapping.rs
    // - map_validation_failure_to_go_format() → moved to adapter/error_mapping.rs
    // - validate_timer_state() → moved to adapter/base.rs
    // - validate_decided_state() → moved to adapter/base.rs
    // - execute_scenario() business logic → moved to adapter/scenario.rs
    
    impl ControllerTest {
        fn run(&self) -> bool {
            // ONLY orchestration logic remains
            let test_context = TestContext {
                test_name: self.name.clone(),
                test_type: TestType::Controller,
                expected_errors: vec![self.expected_error.clone()],
                error_mapping_context: HashMap::new(),
            };
            
            let mut found_expected_error = false;
            
            for (i, run_data) in self.run_instance_data.iter().enumerate() {
                // Create adapter with business logic
                let (adapter, mut scenario_manager) = AdapterBuilder::for_controller_testing_with_context(
                    &committee_member,
                    OperatorId::from(1),
                    scenario_height,
                    shared_current_height,
                    test_context.clone(),
                )?;
                
                // Execute scenario (all business logic in adapter)
                let scenario_result = scenario_manager.execute_controller_scenario(
                    run_data.input_value.clone(),
                    run_data.input_messages.clone().unwrap_or_default(),
                );
                
                // ONLY assertion logic in test
                match self.assert_scenario_result(&scenario_result, run_data) {
                    Ok(()) => eprintln!("Scenario {} passed", i + 1),
                    Err(error_msg) => {
                        if self.is_expected_error(&error_msg) {
                            found_expected_error = true;
                        } else {
                            return false;
                        }
                    }
                }
            }
            
            self.validate_expected_error_handling(found_expected_error)
        }
        
        // ONLY assertion helper methods remain in test
        fn assert_scenario_result(&self, result: &ScenarioResult, expected: &RunInstanceData) -> Result<(), String>;
        fn is_expected_error(&self, error: &str) -> bool;
        fn validate_expected_error_handling(&self, found_expected_error: bool) -> bool;
    }
    ```
  - **create_message.rs Complete Refactoring**:
    ```rust
    impl CreateMessageTest {
        fn run(&self) -> bool {
            let test_context = TestContext {
                test_name: self.name.clone(),
                test_type: TestType::MessageCreation,
                expected_errors: vec![self.expected_error.clone()],
                error_mapping_context: HashMap::new(),
            };
            
            // Create adapter with business logic
            let (adapter, mut scenario_manager) = AdapterBuilder::for_message_creation_with_context(
                &self.committee_member,
                ssv_types::OperatorId::from(self.operator_id),
                test_context,
            )?;
            
            // Execute scenario (all business logic in adapter)
            let scenario_result = scenario_manager.execute_message_creation_scenario(
                self.create_message_request(),
                self.expected_root,
            );
            
            // ONLY assertion logic in test
            self.assert_message_creation_result(&scenario_result)
        }
        
        // ONLY data conversion methods remain in test
        fn create_message_request(&self) -> MessageCreationRequest;
        fn assert_message_creation_result(&self, result: &ScenarioResult) -> bool;
    }
    ```
  - **qbft_message.rs Complete Refactoring**:
    ```rust
    impl QbftMessageTest {
        fn run(&self) -> bool {
            let test_context = TestContext {
                test_name: self.name.clone(),
                test_type: TestType::QbftMessage,
                expected_errors: self.expected_errors(),
                error_mapping_context: HashMap::new(),
            };
            
            // Create adapter with business logic
            let (adapter, mut scenario_manager) = AdapterBuilder::for_validation_testing_with_context(
                &self.committee_member,
                OperatorId::from(1),
                test_context,
            )?;
            
            // Execute validation (all business logic in adapter)
            let scenario_result = scenario_manager.execute_validation_scenario(
                self.create_signed_message(),
            );
            
            // ONLY assertion logic in test
            self.assert_validation_result(&scenario_result)
        }
        
        // ONLY data conversion methods remain in test
        fn create_signed_message(&self) -> SignedSSVMessage;
        fn assert_validation_result(&self, result: &ScenarioResult) -> bool;
    }
    ```
  - **round_robin.rs Complete Refactoring**:
    ```rust
    // Similar pattern: remove all business logic, keep only orchestration and assertions
    ```
  - Integration Requirements:
    - **Remove ALL business logic** from test files (error mapping, validation, scenario execution)
    - **Tests contain ONLY**: data loading, adapter method calls, result assertions
    - **All QBFT logic moved to adapters** with proper error handling
    - **Use ScenarioResult** for all business logic communication between adapters and tests
  - Context: Complete separation of concerns with tests focusing purely on verification

- [ ] **Task #4**: Update adapter module exports
  - Folder: `anchor/spec_tests/src/qbft/adapter/`
  - File: `mod.rs`
  - Enhanced Exports:
    ```rust
    pub mod base;
    pub mod builder;
    pub mod types;
    pub mod processor;
    pub mod validation;
    pub mod error_mapping;
    pub mod scenario;
    
    // Core adapter framework
    pub use base::{QbftTestAdapter, ControllerResult, SharedMessageSender};
    pub use builder::AdapterBuilder;
    
    // Enhanced types with validation support
    pub use types::{
        DecidedState, TimerState, ProcessingResult, AdapterState,
        MessageCreationRequest, ScenarioConfig, AdapterConfig, AdapterError,
        SpecTestCommitteeMember, SpecTestOperator, ValidationResult,
        TestContext, TestType, ScenarioResult
    };
    
    // Business logic modules  
    pub use processor::MessageProcessor;
    pub use scenario::{ScenarioManager, process_controller_scenario_result, process_message_creation_result};
    pub use error_mapping::{ErrorMapper, map_validation_failure_to_go_format, map_with_test_context};
    
    // Enhanced validation with message_validator integration
    pub use validation::{
        validate_message_comprehensive, validate_message_basic_enhanced,
        validate_justifications_enhanced, validate_and_format_errors,
        validate_qbft_message_type
    };
    ```
  - Context: Clean exports for enhanced adapter framework with complete business logic separation

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