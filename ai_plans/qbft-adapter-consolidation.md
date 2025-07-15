# QBFT Adapter Framework Consolidation Implementation Plan

## Executive Summary

> **Problem Statement**: The current QBFT adapter directory contains significant duplication and inconsistent abstractions. Three separate adapters (ControllerAdapter, UnifiedTestAdapter, MessageAdapter) each maintain their own QBFT instances, state tracking, and message processing logic, leading to maintenance burden and interface confusion.

> **Proposed Solution**: Consolidate all adapter functionality into a single, unified `QbftTestAdapter` framework that provides a consistent interface while supporting all current test scenarios (controller testing, message creation, validation).

> **Technical Approach**: Create a unified adapter with shared infrastructure (QBFT instance management, message processing, state tracking) and scenario-specific configuration, eliminating all duplication while maintaining backward compatibility with existing tests.

> **Expected Outcomes**: A single, clean adapter framework that eliminates ~400 lines of duplicated code, provides consistent interfaces, and makes adding new test scenarios straightforward.

## Goals & Objectives

### Primary Goals
- **Eliminate All Duplication**: Remove duplicate QBFT instance creation, TestMessageSender implementations, and state tracking across 3 adapters
- **Unified Interface**: Provide a single, consistent adapter interface that supports all current test scenarios (controller, message creation, validation)

### Secondary Objectives
- **Improved Maintainability**: Changes to QBFT integration happen in one place instead of three
- **Extensibility**: Clean foundation for adding new QBFT test scenarios
- **Clean Breaking Changes**: Remove all legacy interfaces and force migration to unified framework

## Solution Overview

### Approach
Replace the current fragmented adapter structure with a unified `QbftTestAdapter` that contains shared infrastructure and supports scenario-specific configuration through builder patterns and method variants.

### Key Components
1. **QbftTestAdapter**: Single adapter class with unified QBFT instance and state management
2. **AdapterBuilder**: Factory for creating adapters with scenario-specific configuration
3. **UnifiedTypes**: Consistent state, result, and error types across all scenarios
4. **MessageProcessor**: Shared message processing and validation infrastructure

### Architecture Diagram
```
Tests → AdapterBuilder → QbftTestAdapter → QbftCore
                            ↓
                    [Unified State Management]
                            ↓
                    [Shared Message Processing]
```

### Data Flow
```
Test Setup → Builder Config → Adapter Creation → Test Execution → Unified Results
```

### Expected Outcomes
- **Single adapter class** replaces 3 separate implementations
- **Unified interface** for all QBFT test scenarios
- **Shared infrastructure** eliminates duplication
- **Clean test migration** to unified framework with updated interfaces

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
│   ├── mod.rs (Task #4: Clean exports for unified framework)
│   ├── base.rs (Task #1: Core QbftTestAdapter with shared infrastructure)
│   ├── builder.rs (Task #2: AdapterBuilder factory with configuration)
│   ├── types.rs (Task #0: Unified state, result, and error types)
│   ├── processor.rs (Task #1: Shared message processing and validation)
│   └── validation.rs (Task #0: Centralized validation functions)
│
├── controller_test.rs (Task #3: Update to use unified adapter)
├── create_message.rs (Task #3: Update to use unified adapter) 
├── qbft_message.rs (Task #3: Update to use unified adapter)
└── round_robin.rs (Task #3: Update to use unified adapter)
```

### Execution Plan

#### Group A: Foundation Types (Execute all in parallel)
- [x] **Task #0**: Create unified state and result types
  - Folder: `anchor/spec_tests/src/qbft/adapter/`
  - File: `types.rs`
  - Imports:
    - `use ssv_types::{Round, OperatorId, message::SignedSSVMessage}`
    - `use types::Hash256`
    - `use message_validator::ValidationFailure`
    - `use qbft::TestError`
  - Implements:
    ```rust
    #[derive(Debug, Clone)]
    pub struct DecidedState {
        pub decided_count: u64,
        pub decided_value: Option<Vec<u8>>,
    }
    
    #[derive(Debug, Clone)]
    pub struct TimerState {
        pub timeouts: u64,
        pub current_round: Round,
    }
    
    #[derive(Debug, Clone)]
    pub struct ProcessingResult {
        pub consensus_reached: bool,
        pub messages_sent: Vec<SignedSSVMessage>,
        pub validation_errors: Vec<ValidationFailure>,
    }
    
    #[derive(Debug, Clone)]
    pub struct AdapterState {
        pub current_height: u64,
        pub instance_height: u64,
        pub instance_started: bool,
        pub decided_count: u64,
        pub decided_value: Option<Vec<u8>>,
        pub timeout_count: u64,
        pub prepared_state: Option<(Round, Hash256)>,
        pub justifications: Vec<SignedSSVMessage>,
    }
    
    #[derive(Debug, Clone)]
    pub struct MessageCreationRequest {
        pub msg_type: ssv_types::consensus::QbftMessageType,
        pub data_hash: Hash256,
        pub round: Option<Round>,
        pub state_value: Option<Vec<u8>>,
        pub round_change_justifications: Vec<SignedSSVMessage>,
        pub prepare_justifications: Vec<SignedSSVMessage>,
    }
    
    #[derive(Debug, Clone)]
    pub struct ScenarioConfig {
        pub round: Option<Round>,
        pub prepared_state: Option<(Round, Hash256)>,
        pub justifications: Vec<SignedSSVMessage>,
    }
    
    #[derive(Debug, Clone)]
    pub struct AdapterConfig {
        pub instance_height: u64,
        pub current_height: u64,
        pub committee_size: usize,
        pub quorum_threshold: usize,
        pub max_rounds: u64,
    }
    
    #[derive(Debug, thiserror::Error)]
    pub enum AdapterError {
        #[error("QBFT error: {0}")]
        Qbft(#[from] TestError),
        #[error("Validation failed: {0}")]
        Validation(#[from] ValidationFailure),
        #[error("Invalid state: {0}")]
        InvalidState(String),
        #[error("Configuration error: {0}")]
        Config(String),
        #[error("Message creation failed: {0}")]
        MessageCreation(String),
    }
    ```
  - Exports: All types and AdapterError
  - Context: Foundation types used by all adapter components and test files

- [x] **Task #0**: Create centralized validation functions
  - Folder: `anchor/spec_tests/src/qbft/adapter/`
  - File: `validation.rs`
  - Imports:
    - `use ssv_types::message::SignedSSVMessage`
    - `use message_validator::ValidationFailure`
    - `use types::Hash256`
    - `use tree_hash::TreeHash`
  - Implements:
    ```rust
    pub fn validate_message_basic(msg: &SignedSSVMessage) -> Result<(), ValidationFailure> {
        // Basic message structure validation
        // Operator ID validation
        // Signature presence validation
        // Message type validation
    }
    
    pub fn validate_justifications(justifications: &[SignedSSVMessage]) -> Result<(), String> {
        // Validate each justification message
        // Check for proper ordering and structure
        // Validate against expected patterns
    }
    
    pub fn validate_root(message: &SignedSSVMessage, expected: Hash256) -> bool {
        // Compute tree hash root of message
        // Compare against expected value
        message.tree_hash_root() == expected
    }
    
    pub fn extract_committee_from_spec_test(
        committee_member: &super::SpecTestCommitteeMember
    ) -> ssv_types::IndexSet<ssv_types::OperatorId> {
        // Extract committee from JSON spec test data
        // Convert to IndexSet format for QBFT
    }
    ```
  - Exports: All validation functions
  - Context: Shared validation logic used across all adapters and test scenarios

#### Group B: Core Infrastructure (Execute all in parallel after Group A)
- [x] **Task #1**: Create unified QbftTestAdapter with shared infrastructure
  - Folder: `anchor/spec_tests/src/qbft/adapter/`
  - File: `base.rs`
  - Imports:
    - `use std::{collections::VecDeque, sync::Arc}`
    - `use parking_lot::RwLock`
    - `use openssl::pkey::{PKey, Private}`
    - `use qbft::{Qbft, Config, ConfigBuilder, DefaultLeaderFunction, InstanceHeight, MessageSender, UnsignedWrappedQbftMessage}`
    - `use ssv_types::{IndexSet, OperatorId, consensus::BeaconVote, message::SignedSSVMessage, msgid::MessageId}`
    - `use super::types::*`
    - `use super::validation::*`
  - Implements:
    ```rust
    struct SharedMessageSender {
        queue: Arc<RwLock<VecDeque<UnsignedWrappedQbftMessage>>>,
    }
    
    impl MessageSender for SharedMessageSender {
        fn send(&mut self, msg: UnsignedWrappedQbftMessage) {
            self.queue.write().push_back(msg);
        }
    }
    
    pub struct QbftTestAdapter {
        qbft: Qbft<DefaultLeaderFunction, BeaconVote, SharedMessageSender>,
        committee: IndexSet<OperatorId>,
        signing_key: PKey<Private>,
        operator_id: OperatorId,
        identifier: MessageId,
        state: AdapterState,
        message_queue: Arc<RwLock<VecDeque<UnsignedWrappedQbftMessage>>>,
    }
    
    impl QbftTestAdapter {
        pub fn new(
            committee: IndexSet<OperatorId>,
            identifier: MessageId,
            config: AdapterConfig,
            signing_key: PKey<Private>,
            operator_id: OperatorId,
        ) -> Result<Self, AdapterError>;
        
        // Lifecycle management
        pub fn start_instance(&mut self, input_value: Vec<u8>) -> Result<(), AdapterError>;
        pub fn set_current_height(&mut self, height: u64);
        pub fn reset_for_new_scenario(&mut self);
        
        // Message operations
        pub fn create_message(&mut self, request: MessageCreationRequest) -> Result<SignedSSVMessage, AdapterError>;
        pub fn process_messages(&mut self, messages: Vec<SignedSSVMessage>) -> Result<ProcessingResult, AdapterError>;
        
        // Scenario setup
        pub fn setup_scenario(&mut self, config: ScenarioConfig) -> Result<(), AdapterError>;
        
        // State inspection
        pub fn get_decided_state(&self) -> DecidedState;
        pub fn get_timer_state(&self) -> Option<TimerState>;
        pub fn validate_root(&self, message: &SignedSSVMessage, expected: Hash256) -> bool;
        
        // Controller-specific methods
        pub fn process_messages_controller(&mut self, messages: Vec<SignedSSVMessage>) -> Result<ControllerResult, AdapterError>;
        
        // Private helper methods
        fn create_qbft_instance(committee: IndexSet<OperatorId>, config: AdapterConfig, identifier: MessageId) -> Result<Qbft<DefaultLeaderFunction, BeaconVote, SharedMessageSender>, AdapterError>;
        fn update_consensus_state(&mut self);
        fn handle_completed_consensus(&mut self);
    }
    
    // Compatibility type for controller tests
    #[derive(Debug, Clone)]
    pub struct ControllerResult {
        pub processing_result: ProcessingResult,
        pub decided_state: DecidedState,
    }
    ```
  - Exports: QbftTestAdapter, ControllerResult, SharedMessageSender
  - Context: Core unified adapter that replaces ControllerAdapter, UnifiedTestAdapter, and MessageAdapter

- [x] **Task #1**: Create shared message processor
  - Folder: `anchor/spec_tests/src/qbft/adapter/`
  - File: `processor.rs`
  - Imports:
    - `use ssv_types::message::SignedSSVMessage`
    - `use message_validator::ValidationFailure`
    - `use super::types::*`
    - `use super::validation::*`
  - Implements:
    ```rust
    pub struct MessageProcessor {
        validation_errors: Vec<ValidationFailure>,
    }
    
    impl MessageProcessor {
        pub fn new() -> Self;
        
        pub fn process_message_batch(
            &mut self,
            messages: Vec<SignedSSVMessage>,
            qbft: &mut qbft::Qbft<qbft::DefaultLeaderFunction, ssv_types::consensus::BeaconVote, super::base::SharedMessageSender>,
        ) -> Result<ProcessingResult, AdapterError>;
        
        pub fn validate_and_filter(
            &mut self,
            messages: Vec<SignedSSVMessage>
        ) -> (Vec<SignedSSVMessage>, Vec<ValidationFailure>);
        
        pub fn extract_outgoing_messages(
            &self,
            message_queue: &Arc<parking_lot::RwLock<std::collections::VecDeque<qbft::UnsignedWrappedQbftMessage>>>
        ) -> Vec<SignedSSVMessage>;
        
        pub fn get_validation_errors(&self) -> &[ValidationFailure];
        pub fn clear_validation_errors(&mut self);
    }
    ```
  - Exports: MessageProcessor
  - Context: Shared message processing logic used by unified adapter for all test scenarios

- [x] **Task #2**: Create adapter builder factory
  - Folder: `anchor/spec_tests/src/qbft/adapter/`
  - File: `builder.rs`
  - Imports:
    - `use openssl::pkey::PKey`
    - `use ssv_types::{IndexSet, OperatorId, msgid::MessageId}`
    - `use super::types::*`
    - `use super::base::QbftTestAdapter`
    - `use super::validation::extract_committee_from_spec_test`
    - `use crate::utils::test_keys::TestKeySet`
  - Implements:
    ```rust
    pub struct AdapterBuilder {
        committee: Option<IndexSet<OperatorId>>,
        identifier: Option<MessageId>,
        config: Option<AdapterConfig>,
        signing_key: Option<PKey<openssl::pkey::Private>>,
        operator_id: Option<OperatorId>,
    }
    
    impl AdapterBuilder {
        pub fn new() -> Self;
        
        // Configuration methods
        pub fn with_spec_test_data(
            mut self,
            committee_member: &SpecTestCommitteeMember,
            operator_id: OperatorId,
        ) -> Self;
        
        pub fn with_committee(mut self, committee: IndexSet<OperatorId>) -> Self;
        pub fn with_identifier(mut self, identifier: MessageId) -> Self;
        pub fn with_config(mut self, config: AdapterConfig) -> Self;
        pub fn with_signing_key(mut self, key: PKey<openssl::pkey::Private>) -> Self;
        pub fn with_operator_id(mut self, operator_id: OperatorId) -> Self;
        
        // Convenience configuration for common scenarios
        pub fn for_controller_test(mut self, instance_height: u64, current_height: u64) -> Self;
        pub fn for_message_creation_test(mut self) -> Self;
        pub fn for_validation_test(mut self) -> Self;
        
        // Build method
        pub fn build(self) -> Result<QbftTestAdapter, AdapterError>;
        
        // Static convenience methods (clean new interface)
        pub fn for_message_creation(
            committee_member: &SpecTestCommitteeMember,
            operator_id: OperatorId,
        ) -> Result<QbftTestAdapter, AdapterError>;
        
        pub fn for_controller_testing(
            committee_member: &SpecTestCommitteeMember,
            operator_id: OperatorId,
            instance_height: u64,
            current_height: u64,
        ) -> Result<QbftTestAdapter, AdapterError>;
    }
    
    impl Default for AdapterBuilder {
        fn default() -> Self { Self::new() }
    }
    ```
  - Exports: AdapterBuilder
  - Context: Factory for creating unified adapters with scenario-specific configuration, replacing old AdapterFactory

#### Group C: Integration Updates (Execute all in parallel after Group B)
- [x] **Task #3**: Update all test files to use unified adapter
  - Files: `controller_test.rs`, `create_message.rs`, `qbft_message.rs`, `round_robin.rs`
  - Imports to Update:
    - Replace `use super::adapter::UnifiedTestAdapter` with `use super::adapter::{QbftTestAdapter, AdapterBuilder}`
    - Replace `use super::adapter::{MessageAdapter, AdapterFactory}` with `use super::adapter::{QbftTestAdapter, AdapterBuilder}`
  - **controller_test.rs Complete Rewrite**:
    ```rust
    // Replace create_controller_adapter_with_height method:
    fn create_controller_adapter_with_height(
        &self,
        instance_height: u64,
        current_height: u64,
    ) -> Result<QbftTestAdapter, String> {
        AdapterBuilder::for_controller_testing(
            &self.committee_member,
            OperatorId::from(1),
            instance_height,
            current_height,
        ).map_err(|e| format!("Failed to create adapter: {}", e))
    }
    
    // Update all method calls:
    // - Replace process_messages_controller() with process_messages()
    // - Update error handling to use new AdapterError types
    // - Use new ProcessingResult and ControllerResult types
    // - Remove any legacy compatibility code
    ```
  - **create_message.rs Complete Rewrite**:
    ```rust
    // Replace adapter creation in run() method:
    let mut adapter = AdapterBuilder::for_message_creation(
        &self.committee_member, 
        ssv_types::OperatorId::from(self.operator_id)
    ).map_err(|e| e.to_string())?;
    
    // Update method calls:
    // - Replace setup_scenario() parameters with ScenarioConfig struct
    // - Replace create_message() parameters with MessageCreationRequest struct
    // - Update error handling to use AdapterError
    // - Remove legacy factory pattern usage
    ```
  - **qbft_message.rs Complete Rewrite**:
    ```rust
    // Replace all validation calls:
    use super::adapter::{QbftTestAdapter, AdapterBuilder, validate_message_basic};
    
    // Create adapter for validation testing:
    let adapter = AdapterBuilder::new()
        .for_validation_test()
        .build()?;
    
    // Use unified validation interface
    ```
  - **round_robin.rs Complete Rewrite**:
    ```rust
    // Replace any adapter usage with unified builder pattern:
    let adapter = AdapterBuilder::for_controller_testing(...)?;
    
    // Update all method calls to use unified interface
    // Remove any legacy adapter references
    ```
  - Integration Requirements:
    - **BREAKING CHANGES REQUIRED**: All test files must be updated to new interfaces
    - **New error types**: Update error handling to use AdapterError
    - **New result types**: Update result handling to use ProcessingResult, ControllerResult
    - **Clean interfaces**: Remove all legacy method signatures and compatibility code
  - Context: Complete rewrite of all test files to use clean unified adapter interface with breaking changes

- [x] **Task #4**: Update adapter module exports and cleanup
  - Folder: `anchor/spec_tests/src/qbft/adapter/`
  - File: `mod.rs`
  - Remove Files: `controller.rs`, `unified_controller.rs`, `message.rs`, `factory.rs` (old implementations)
  - New Imports:
    ```rust
    pub mod base;
    pub mod builder;
    pub mod types;
    pub mod processor;
    pub mod validation;
    
    // Re-export types from the deleted files for compatibility
    pub use types::{SpecTestCommitteeMember, SpecTestOperator};
    ```
  - New Exports:
    ```rust
    // Core unified adapter
    pub use base::{QbftTestAdapter, ControllerResult};
    
    // Builder and factory
    pub use builder::AdapterBuilder;
    
    // Types and errors
    pub use types::{
        DecidedState, TimerState, ProcessingResult, AdapterState,
        MessageCreationRequest, ScenarioConfig, AdapterConfig, AdapterError
    };
    
    // Validation functions
    pub use validation::{validate_message_basic, validate_justifications, validate_root};
    ```
  - File Cleanup:
    - Delete `controller.rs` (replaced by `base.rs`)
    - Delete `unified_controller.rs` (replaced by `base.rs`)
    - Delete `message.rs` (replaced by `base.rs`)  
    - Delete `factory.rs` (replaced by `builder.rs`)
  - Context: Clean module structure with unified exports, removing all legacy compatibility

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