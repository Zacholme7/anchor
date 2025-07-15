# QBFT Spec Tests Refactoring Implementation Plan

## Executive Summary

> **Problem**: The current QBFT spec tests contain significant code duplication, custom validation logic, scattered deserializers, and a monolithic 1,241-line UnifiedTestAdapter. This makes the code hard to maintain and inconsistent with the existing infrastructure.
>
> **Solution**: Refactor the QBFT spec tests to leverage existing validation infrastructure (`message_validator`), consolidate deserializers into the existing `utils/deserializers.rs` framework, break down the monolithic adapter, and create shared utilities to eliminate duplication.
>
> **Technical Approach**: Extract common components into shared modules, leverage existing `ValidationFailure` error types, consolidate deserializers, and create a clean facade over the existing validation infrastructure.
>
> **Expected Outcomes**: 30-40% reduction in code duplication, consistent error handling, professional code organization, and improved maintainability while preserving all existing test functionality.

## Goals & Objectives

### Primary Goals
- **Eliminate Code Duplication**: Reduce the 30-40% code duplication in committee setup, key management, and message creation patterns
- **Leverage Existing Infrastructure**: Use `message_validator` crate and `utils/deserializers.rs` instead of custom validation and deserialization
- **Create Professional Code Organization**: Break down the 1,241-line monolithic adapter into focused, maintainable modules

### Secondary Objectives
- **Standardize Error Handling**: Use existing `ValidationFailure` types and `thiserror` patterns consistently
- **Improve Test Infrastructure**: Create reusable test utilities and builders
- **Enhance Code Quality**: Follow existing codebase patterns and Rust best practices

## Solution Overview

### Approach
Refactor the QBFT spec tests by leveraging existing `common/` infrastructure (`ssv_types`, `qbft`, `message_validator`), consolidating deserializers, and focusing on spec test execution logic. Eliminate duplicate type definitions by using existing robust types and focus effort on Go test compatibility.

### Key Components
1. **Leverage Existing Types**: Use `ssv_types::CommitteeInfo`, `qbft::TestConfig`, `qbft::Qbft` instead of custom types
2. **Consolidated Deserializers**: Move all QBFT deserializers to `utils/deserializers.rs` framework
3. **Use Existing QBFT Implementation**: Use `qbft::Qbft` for message creation and consensus logic
4. **Simplified Test Adapter**: Thin wrapper over existing `qbft::Qbft` and `ValidationAdapter`
5. **Focus on Spec Test Logic**: Concentrate on JSON parsing, test execution, and Go compatibility

### Architecture Diagram
```
qbft/
├── common_types.rs     (Shared type definitions)
├── test_utils.rs       (Reusable test infrastructure)
├── validation_facade.rs (Clean validation API)
├── error.rs           (Unified error handling)
├── adapters/
│   ├── mod.rs         (Adapter coordination)
│   ├── message.rs     (Message creation logic)
│   ├── validation.rs  (Validation logic)
│   └── consensus.rs   (Consensus detection)
└── tests/
    ├── create_message.rs
    ├── controller_test.rs
    ├── qbft_message.rs
    └── round_robin.rs
```

### Data Flow
```
JSON Test Data → Extended Deserializers → Shared Types → Test Utilities → Validation Facade → message_validator
```

### Expected Outcomes
- 30-40% reduction in code duplication across QBFT spec tests
- Consistent error handling using existing `ValidationFailure` types
- Professional code organization with focused, maintainable modules
- Improved test reliability through shared infrastructure
- Better integration with existing validation and deserialization systems

## Implementation Tasks

### CRITICAL IMPLEMENTATION RULES
1. **NO PLACEHOLDER CODE**: Every implementation must be production-ready
2. **COMPLETE IMPLEMENTATIONS**: Each task must fully implement its feature including all consumers and integration points
3. **LEVERAGE EXISTING INFRASTRUCTURE**: Use `message_validator`, `utils/deserializers.rs`, and existing patterns
4. **MAINTAIN BACKWARD COMPATIBILITY**: All existing tests must continue to pass
5. **FOLLOW EXISTING PATTERNS**: Use `thiserror`, `SpecTest` trait, and existing code organization

### Visual Dependency Tree
```
anchor/spec_tests/src/
├── utils/
│   └── deserializers.rs (Task #0: Extend with QBFT deserializers)
│
├── qbft/
│   ├── common_types.rs (Task #1: Shared type definitions)
│   ├── error.rs (Task #2: Unified error handling)
│   ├── test_utils.rs (Task #3: Reusable test infrastructure)
│   ├── validation_facade.rs (Task #4: Clean validation API)
│   │
│   ├── adapters/
│   │   ├── mod.rs (Task #5: Adapter coordination)
│   │   ├── message.rs (Task #5: Message creation logic)
│   │   ├── validation.rs (Task #5: Validation logic)
│   │   └── consensus.rs (Task #5: Consensus detection)
│   │
│   └── tests/
│       ├── create_message.rs (Task #6: Refactor create message tests)
│       ├── controller_test.rs (Task #6: Refactor controller tests)
│       ├── qbft_message.rs (Task #6: Refactor message tests)
│       └── round_robin.rs (Task #6: Refactor round robin tests)
│
└── lib.rs (Task #7: Update module exports and registration)
```

### Execution Plan

#### Group A: Foundation Infrastructure (Execute all in parallel)
- [x] **Task #0**: Extend utils/deserializers.rs with QBFT deserializers
  - **Folder**: `anchor/spec_tests/src/utils/`
  - **File**: `deserializers.rs`
  - **Add Module**: `qbft_deserializers` within existing structure
  - **Implements**:
    ```rust
    pub mod qbft_deserializers {
        use super::*;
        use ssv_types::{consensus::QbftMessageType, Round};
        use types::Hash256;
        
        pub fn deserialize_qbft_message_type<'de, D>(
            deserializer: D,
        ) -> Result<QbftMessageType, D::Error>
        where
            D: Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            match s.as_str() {
                "createProposal" => Ok(QbftMessageType::Proposal),
                "CreatePrepare" => Ok(QbftMessageType::Prepare),
                "CreateCommit" => Ok(QbftMessageType::Commit),
                "CreateRoundChange" => Ok(QbftMessageType::RoundChange),
                _ => Err(serde::de::Error::custom(format!(
                    "Invalid QBFT message type: '{s}'"
                ))),
            }
        }
        
        pub fn deserialize_value_into_root<'de, D>(
            deserializer: D,
        ) -> Result<Hash256, D::Error>
        where
            D: Deserializer<'de>,
        {
            let bytes = <Vec<u8>>::deserialize(deserializer)?;
            if bytes.len() != 32 {
                return Err(serde::de::Error::custom(format!(
                    "Invalid Value length: {} bytes (expected 32)",
                    bytes.len()
                )));
            }
            Ok(Hash256::from_slice(&bytes))
        }
        
        pub fn deserialize_u64_into_round<'de, D>(
            deserializer: D,
        ) -> Result<Option<Round>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let round = <u64>::deserialize(deserializer)?;
            if round == 0 {
                Ok(None)
            } else {
                Ok(Some(round.into()))
            }
        }
        
        pub fn deserialize_committee_member<'de, D>(
            deserializer: D,
        ) -> Result<CommitteeMember, D::Error>
        where
            D: Deserializer<'de>,
        {
            #[derive(Deserialize)]
            struct CommitteeMemberHelper {
                #[serde(rename = "OperatorID")]
                operator_id: u64,
                #[serde(rename = "CommitteeID")]
                committee_id: Vec<u8>,
                #[serde(rename = "SSVOperatorPubKey")]
                ssv_operator_pub_key: String,
                #[serde(rename = "FaultyNodes")]
                faulty_nodes: u64,
                #[serde(rename = "Committee")]
                committee: Vec<OperatorHelper>,
                #[serde(rename = "DomainType")]
                domain_type: [u8; 4],
            }
            
            #[derive(Deserialize)]
            struct OperatorHelper {
                #[serde(rename = "OperatorID")]
                operator_id: u64,
                #[serde(rename = "SSVOperatorPubKey")]
                ssv_operator_pub_key: String,
            }
            
            let helper = CommitteeMemberHelper::deserialize(deserializer)?;
            Ok(CommitteeMember {
                operator_id: OperatorId::from(helper.operator_id),
                committee_id: helper.committee_id,
                ssv_operator_pub_key: helper.ssv_operator_pub_key,
                faulty_nodes: helper.faulty_nodes,
                committee: helper.committee.into_iter().map(|op| Operator {
                    operator_id: OperatorId::from(op.operator_id),
                    ssv_operator_pub_key: op.ssv_operator_pub_key,
                }).collect(),
                domain_type: helper.domain_type,
            })
        }
    }
    ```
  - **Integration**: Used by all QBFT test structs for consistent deserialization
  - **Context**: Consolidates all QBFT deserializers in the existing framework

- [x] **Task #1**: Create shared QBFT type definitions
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `common_types.rs`
  - **Implements**:
    ```rust
    use serde::Deserialize;
    use ssv_types::OperatorId;
    
    /// Committee member structure used across all QBFT tests
    #[derive(Debug, Clone, Deserialize)]
    pub struct CommitteeMember {
        pub operator_id: OperatorId,
        pub committee_id: Vec<u8>,
        pub ssv_operator_pub_key: String,
        pub faulty_nodes: u64,
        pub committee: Vec<Operator>,
        pub domain_type: [u8; 4],
    }
    
    /// Operator structure used in committee definitions
    #[derive(Debug, Clone, Deserialize)]
    pub struct Operator {
        pub operator_id: OperatorId,
        pub ssv_operator_pub_key: String,
    }
    
    /// Configuration for QBFT test scenarios
    #[derive(Debug, Clone)]
    pub struct QbftTestConfig {
        pub committee_size: usize,
        pub quorum_threshold: usize,
        pub max_rounds: u64,
        pub instance_height: u64,
        pub fault_tolerance: usize,
    }
    
    impl QbftTestConfig {
        pub fn four_node_committee() -> Self {
            Self {
                committee_size: 4,
                quorum_threshold: 3,
                max_rounds: 100,
                instance_height: 0,
                fault_tolerance: 1,
            }
        }
        
        pub fn from_committee_size(size: usize) -> Self {
            Self {
                committee_size: size,
                quorum_threshold: (size * 2 / 3) + 1,
                max_rounds: 100,
                instance_height: 0,
                fault_tolerance: size / 3,
            }
        }
    }
    
    /// Test scenario configuration
    #[derive(Debug, Clone)]
    pub struct QbftTestScenario {
        pub round: ssv_types::Round,
        pub last_prepared_round: Option<ssv_types::Round>,
        pub last_prepared_value: Option<types::Hash256>,
        pub round_change_justifications: Vec<ssv_types::message::SignedSSVMessage>,
        pub prepare_justifications: Vec<ssv_types::message::SignedSSVMessage>,
    }
    
    impl Default for QbftTestScenario {
        fn default() -> Self {
            Self {
                round: 1.into(),
                last_prepared_round: None,
                last_prepared_value: None,
                round_change_justifications: Vec::new(),
                prepare_justifications: Vec::new(),
            }
        }
    }
    ```
  - **Exports**: `CommitteeMember`, `Operator`, `QbftTestConfig`, `QbftTestScenario`
  - **Integration**: Used by all QBFT test modules and adapters
  - **Context**: Eliminates type duplication across test files

- [x] **Task #2**: Create unified error handling
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `error.rs`
  - **Implements**:
    ```rust
    use thiserror::Error;
    use anchor_message_validator::ValidationFailure;
    use crate::TestError;
    
    /// Unified error type for QBFT spec tests
    #[derive(Debug, Error)]
    pub enum QbftSpecTestError {
        #[error("Validation failed: {0}")]
        ValidationFailure(#[from] ValidationFailure),
        
        #[error("Test error: {0}")]
        TestError(#[from] TestError),
        
        #[error("Setup error: {0}")]
        SetupError(String),
        
        #[error("Deserialization error: {0}")]
        DeserializationError(String),
        
        #[error("Configuration error: {0}")]
        ConfigurationError(String),
        
        #[error("Committee error: {0}")]
        CommitteeError(String),
        
        #[error("Key management error: {0}")]
        KeyManagementError(String),
        
        #[error("Message creation error: {0}")]
        MessageCreationError(String),
        
        #[error("Consensus error: {0}")]
        ConsensusError(String),
    }
    
    /// Result type for QBFT spec tests
    pub type QbftSpecTestResult<T> = Result<T, QbftSpecTestError>;
    
    /// Convert from various error types to QbftSpecTestError
    impl From<serde_json::Error> for QbftSpecTestError {
        fn from(err: serde_json::Error) -> Self {
            QbftSpecTestError::DeserializationError(err.to_string())
        }
    }
    
    impl From<base64::DecodeError> for QbftSpecTestError {
        fn from(err: base64::DecodeError) -> Self {
            QbftSpecTestError::DeserializationError(format!("Base64 decode error: {}", err))
        }
    }
    
    impl From<openssl::error::ErrorStack> for QbftSpecTestError {
        fn from(err: openssl::error::ErrorStack) -> Self {
            QbftSpecTestError::KeyManagementError(err.to_string())
        }
    }
    ```
  - **Exports**: `QbftSpecTestError`, `QbftSpecTestResult`
  - **Integration**: Used by all QBFT modules for consistent error handling
  - **Context**: Leverages existing `ValidationFailure` types and `thiserror` patterns

#### Group B: Test Infrastructure (Execute all in parallel after Group A)
- [x] **Task #3**: Create reusable test infrastructure
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `test_utils.rs`
  - **Imports**:
    ```rust
    use super::{common_types::*, error::*};
    use crate::utils::test_keys::TestKeySet;
    use ssv_types::{OperatorId, IndexSet, msgid::MessageId};
    use openssl::pkey::{PKey, Private};
    use std::collections::HashMap;
    ```
  - **Implements**:
    ```rust
    /// Builder for QBFT test infrastructure
    pub struct QbftTestBuilder {
        committee: Option<IndexSet<OperatorId>>,
        config: Option<QbftTestConfig>,
        committee_member: Option<CommitteeMember>,
        operator_id: Option<OperatorId>,
        identifier: Option<MessageId>,
        signing_keys: HashMap<OperatorId, PKey<Private>>,
    }
    
    impl QbftTestBuilder {
        pub fn new() -> Self {
            Self {
                committee: None,
                config: None,
                committee_member: None,
                operator_id: None,
                identifier: None,
                signing_keys: HashMap::new(),
            }
        }
        
        pub fn with_four_node_committee(mut self) -> Self {
            let committee = [1, 2, 3, 4]
                .iter()
                .map(|&id| OperatorId::from(id))
                .collect();
            self.committee = Some(committee);
            self.config = Some(QbftTestConfig::four_node_committee());
            self
        }
        
        pub fn with_committee_member(mut self, committee_member: CommitteeMember) -> Self {
            let committee = committee_member.committee
                .iter()
                .map(|op| op.operator_id)
                .collect();
            self.committee = Some(committee);
            self.config = Some(QbftTestConfig::from_committee_size(committee_member.committee.len()));
            self.committee_member = Some(committee_member);
            self
        }
        
        pub fn with_operator_id(mut self, operator_id: OperatorId) -> Self {
            self.operator_id = Some(operator_id);
            self
        }
        
        pub fn with_identifier(mut self, identifier: MessageId) -> Self {
            self.identifier = Some(identifier);
            self
        }
        
        pub fn build(self) -> QbftSpecTestResult<QbftTestInstance> {
            let committee = self.committee.unwrap_or_else(|| {
                [1, 2, 3, 4]
                    .iter()
                    .map(|&id| OperatorId::from(id))
                    .collect()
            });
            
            let config = self.config.unwrap_or_else(|| QbftTestConfig::four_node_committee());
            
            let identifier = self.identifier.unwrap_or_else(|| MessageId::for_spectest());
            
            let operator_id = self.operator_id.unwrap_or_else(|| OperatorId::from(1));
            
            // Setup signing keys
            let signing_keys = self.setup_signing_keys(&committee)?;
            
            let signing_key = signing_keys.get(&operator_id)
                .ok_or_else(|| QbftSpecTestError::KeyManagementError(
                    format!("No signing key found for operator {}", operator_id)
                ))?
                .clone();
            
            Ok(QbftTestInstance {
                committee,
                config,
                identifier,
                operator_id,
                signing_key,
                committee_member: self.committee_member,
            })
        }
        
        fn setup_signing_keys(&self, committee: &IndexSet<OperatorId>) -> QbftSpecTestResult<HashMap<OperatorId, PKey<Private>>> {
            let test_keys = TestKeySet::four_share_set();
            let mut signing_keys = HashMap::new();
            
            for operator_id in committee {
                let rsa_key = test_keys.operator_keys.get(operator_id)
                    .ok_or_else(|| QbftSpecTestError::KeyManagementError(
                        format!("No test key found for operator {}", operator_id)
                    ))?;
                
                let pkey = PKey::from_rsa(rsa_key.clone())?;
                signing_keys.insert(*operator_id, pkey);
            }
            
            Ok(signing_keys)
        }
    }
    
    /// QBFT test instance with all required components
    pub struct QbftTestInstance {
        pub committee: IndexSet<OperatorId>,
        pub config: QbftTestConfig,
        pub identifier: MessageId,
        pub operator_id: OperatorId,
        pub signing_key: PKey<Private>,
        pub committee_member: Option<CommitteeMember>,
    }
    
    impl QbftTestInstance {
        pub fn builder() -> QbftTestBuilder {
            QbftTestBuilder::new()
        }
        
        pub fn four_node_default() -> QbftSpecTestResult<Self> {
            Self::builder().with_four_node_committee().build()
        }
        
        pub fn from_committee_member(committee_member: CommitteeMember) -> QbftSpecTestResult<Self> {
            Self::builder().with_committee_member(committee_member).build()
        }
    }
    
    /// Utilities for committee setup
    pub mod committee_utils {
        use super::*;
        
        pub fn create_four_node_committee() -> IndexSet<OperatorId> {
            [1, 2, 3, 4]
                .iter()
                .map(|&id| OperatorId::from(id))
                .collect()
        }
        
        pub fn create_committee_from_member(committee_member: &CommitteeMember) -> IndexSet<OperatorId> {
            committee_member.committee
                .iter()
                .map(|op| op.operator_id)
                .collect()
        }
        
        pub fn parse_message_id_from_base64(id_str: &str) -> QbftSpecTestResult<MessageId> {
            use base64::Engine;
            
            let decoded = base64::engine::general_purpose::STANDARD.decode(id_str)?;
            
            if decoded.len() != 56 {
                return Err(QbftSpecTestError::DeserializationError(
                    format!("Invalid identifier length: expected 56, got {}", decoded.len())
                ));
            }
            
            let mut id_bytes = [0u8; 56];
            id_bytes.copy_from_slice(&decoded);
            Ok(MessageId::from(id_bytes))
        }
    }
    ```
  - **Exports**: `QbftTestBuilder`, `QbftTestInstance`, `committee_utils`
  - **Integration**: Used by all QBFT test modules to eliminate setup duplication
  - **Context**: Creates reusable infrastructure for committee setup and key management

- [x] **Task #4**: Create validation facade
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `validation_facade.rs`
  - **Imports**:
    ```rust
    use super::{common_types::*, error::*};
    use crate::qbft::validation_adapter::ValidationAdapter;
    use anchor_message_validator::{ValidationFailure, ValidationResult};
    use ssv_types::message::SignedSSVMessage;
    use std::collections::HashMap;
    ```
  - **Implements**:
    ```rust
    /// Facade for QBFT validation using existing infrastructure
    pub struct QbftValidationFacade {
        validation_adapter: ValidationAdapter,
    }
    
    impl QbftValidationFacade {
        pub fn new() -> QbftSpecTestResult<Self> {
            let validation_adapter = ValidationAdapter::new()
                .map_err(|e| QbftSpecTestError::SetupError(format!("Failed to create validation adapter: {}", e)))?;
            
            Ok(Self {
                validation_adapter,
            })
        }
        
        /// Validate a QBFT message using the existing validation infrastructure
        pub fn validate_message(&self, message: &SignedSSVMessage) -> QbftSpecTestResult<()> {
            match self.validation_adapter.validate_message(message) {
                Ok(_) => Ok(()),
                Err(validation_failure) => Err(QbftSpecTestError::ValidationFailure(validation_failure)),
            }
        }
        
        /// Validate QBFT-specific semantics
        pub fn validate_qbft_semantics(&self, message: &SignedSSVMessage) -> QbftSpecTestResult<()> {
            // Use existing validation adapter for QBFT-specific validation
            self.validation_adapter.validate_qbft_semantics(message)
                .map_err(|e| QbftSpecTestError::ValidationFailure(e))
        }
        
        /// Validate message root matches expected
        pub fn validate_message_root(&self, message: &SignedSSVMessage, expected_root: types::Hash256) -> QbftSpecTestResult<bool> {
            let actual_root = message.tree_hash_root();
            Ok(actual_root == expected_root)
        }
        
        /// Comprehensive validation combining all checks
        pub fn validate_comprehensive(&self, message: &SignedSSVMessage, expected_root: Option<types::Hash256>) -> QbftSpecTestResult<bool> {
            // First validate the message itself
            self.validate_message(message)?;
            
            // Then validate QBFT semantics
            self.validate_qbft_semantics(message)?;
            
            // Finally validate root if provided
            if let Some(expected_root) = expected_root {
                return self.validate_message_root(message, expected_root);
            }
            
            Ok(true)
        }
    }
    
    impl Default for QbftValidationFacade {
        fn default() -> Self {
            Self::new().expect("Failed to create default validation facade")
        }
    }
    ```
  - **Exports**: `QbftValidationFacade`
  - **Integration**: Used by all QBFT tests for consistent validation
  - **Context**: Provides clean API over existing validation infrastructure

#### Group C: Adapter Refactoring (Execute all in parallel after Group B)
- [x] **Task #5**: Refactor UnifiedTestAdapter into focused modules
  - **Folder**: `anchor/spec_tests/src/qbft/adapters/`
  - **Files**: `mod.rs`, `message.rs`, `validation.rs`, `consensus.rs`
  - **Creates Modular Structure**:
    ```rust
    // mod.rs - Adapter coordination
    pub mod message;
    pub mod validation;
    pub mod consensus;
    
    use super::{common_types::*, error::*, test_utils::*, validation_facade::*};
    use message::MessageAdapter;
    use validation::ValidationAdapter;
    use consensus::ConsensusAdapter;
    
    /// Refactored unified adapter with focused modules
    pub struct UnifiedTestAdapter {
        message_adapter: MessageAdapter,
        validation_adapter: ValidationAdapter,
        consensus_adapter: ConsensusAdapter,
        test_instance: QbftTestInstance,
    }
    
    impl UnifiedTestAdapter {
        pub fn new(committee: IndexSet<OperatorId>, identifier: MessageId, config: QbftTestConfig) -> QbftSpecTestResult<Self> {
            let test_instance = QbftTestInstance::builder()
                .with_committee(committee)
                .with_identifier(identifier)
                .with_config(config)
                .build()?;
            
            let message_adapter = MessageAdapter::new(&test_instance)?;
            let validation_adapter = ValidationAdapter::new(&test_instance)?;
            let consensus_adapter = ConsensusAdapter::new(&test_instance)?;
            
            Ok(Self {
                message_adapter,
                validation_adapter,
                consensus_adapter,
                test_instance,
            })
        }
        
        pub fn create_message(&mut self, /* params */) -> QbftSpecTestResult<SignedSSVMessage> {
            self.message_adapter.create_message(/* params */)
        }
        
        pub fn validate_message(&self, message: &SignedSSVMessage) -> QbftSpecTestResult<()> {
            self.validation_adapter.validate_message(message)
        }
        
        pub fn detect_consensus(&self, messages: &[SignedSSVMessage]) -> QbftSpecTestResult<bool> {
            self.consensus_adapter.detect_consensus(messages)
        }
        
        pub fn setup_test_scenario(&mut self, scenario: QbftTestScenario) -> QbftSpecTestResult<()> {
            self.message_adapter.setup_scenario(&scenario)?;
            self.validation_adapter.setup_scenario(&scenario)?;
            self.consensus_adapter.setup_scenario(&scenario)?;
            Ok(())
        }
    }
    ```
  - **message.rs** - Message creation logic extracted from UnifiedTestAdapter
  - **validation.rs** - Validation logic using QbftValidationFacade
  - **consensus.rs** - Consensus detection logic
  - **Integration**: Used by all QBFT tests as drop-in replacement for monolithic adapter
  - **Context**: Breaks down 1,241-line monolithic adapter into focused modules

#### Group D: Test Refactoring (Execute all in parallel after Group C)
- [x] **Task #6**: Refactor individual test modules
  - **Folders**: `anchor/spec_tests/src/qbft/tests/`
  - **Files**: `create_message.rs`, `controller_test.rs`, `qbft_message.rs`, `round_robin.rs`
  - **Refactors Each Test Module**:
    ```rust
    // create_message.rs - Updated to use shared infrastructure
    use super::super::{common_types::*, error::*, test_utils::*, validation_facade::*};
    use crate::utils::deserializers::qbft_deserializers::*;
    use crate::{QbftSpecTestType, SpecTest, SpecTestType};
    
    #[derive(Deserialize)]
    pub struct CreateMessageTest {
        #[serde(rename = "Name")]
        pub name: String,
        
        #[serde(rename = "Value", deserialize_with = "deserialize_value_into_root")]
        pub root: Hash256,
        
        #[serde(rename = "Round", deserialize_with = "deserialize_u64_into_round")]
        pub round: Option<Round>,
        
        #[serde(rename = "CreateType", deserialize_with = "deserialize_qbft_message_type")]
        pub create_type: QbftMessageType,
        
        #[serde(rename = "ExpectedRoot")]
        pub expected_root: Hash256,
        
        #[serde(rename = "ExpectedError")]
        pub expected_error: String,
        
        #[serde(rename = "CommitteeMember", deserialize_with = "deserialize_committee_member")]
        pub committee_member: Option<CommitteeMember>,
        
        #[serde(rename = "Identifier")]
        pub identifier: Option<String>,
        
        #[serde(rename = "OperatorID")]
        pub operator_id: Option<u64>,
        
        // ... other fields using consolidated deserializers
    }
    
    impl SpecTest for CreateMessageTest {
        fn name(&self) -> &str {
            &self.name
        }
        
        fn run(&self) -> bool {
            match self.run_test() {
                Ok(success) => success,
                Err(e) => {
                    eprintln!("Test failed: {}", e);
                    false
                }
            }
        }
        
        fn setup(&mut self) {
            // Setup is handled in run_test()
        }
        
        fn test_type() -> SpecTestType {
            SpecTestType::Qbft(QbftSpecTestType::CreateMessage)
        }
    }
    
    impl CreateMessageTest {
        fn run_test(&self) -> QbftSpecTestResult<bool> {
            // Create test instance using shared infrastructure
            let test_instance = if let Some(committee_member) = &self.committee_member {
                QbftTestInstance::from_committee_member(committee_member.clone())?
            } else {
                QbftTestInstance::four_node_default()?
            };
            
            // Create adapter using refactored modules
            let mut adapter = UnifiedTestAdapter::new(
                test_instance.committee.clone(),
                test_instance.identifier,
                test_instance.config.clone()
            )?;
            
            // Setup test scenario
            let scenario = self.create_test_scenario()?;
            adapter.setup_test_scenario(scenario)?;
            
            // Create and validate message
            let message = adapter.create_message(
                self.create_type,
                self.root,
                self.round,
                /* other params */
            )?;
            
            // Use validation facade for consistent validation
            let validation_facade = QbftValidationFacade::new()?;
            let root_matches = validation_facade.validate_message_root(&message, self.expected_root)?;
            
            Ok(root_matches)
        }
        
        fn create_test_scenario(&self) -> QbftSpecTestResult<QbftTestScenario> {
            // Use shared scenario builder
            Ok(QbftTestScenario {
                round: self.round.unwrap_or(1.into()),
                // ... other fields
            })
        }
    }
    ```
  - **Integration**: Each test module updated to use shared infrastructure
  - **Context**: Eliminates code duplication while maintaining all test functionality

#### Group E: Integration (Execute after all previous groups)
- [x] **Task #7**: Update module exports and test registration
  - **Folders**: `anchor/spec_tests/src/`
  - **Files**: `lib.rs`, `qbft/mod.rs`
  - **Updates Module Structure**:
    ```rust
    // qbft/mod.rs - Updated module exports
    pub mod common_types;
    pub mod error;
    pub mod test_utils;
    pub mod validation_facade;
    pub mod adapters;
    pub mod tests;
    
    // Re-export main types
    pub use common_types::{CommitteeMember, Operator, QbftTestConfig, QbftTestScenario};
    pub use error::{QbftSpecTestError, QbftSpecTestResult};
    pub use test_utils::{QbftTestBuilder, QbftTestInstance, committee_utils};
    pub use validation_facade::QbftValidationFacade;
    pub use adapters::UnifiedTestAdapter;
    
    // Re-export test types
    pub use tests::create_message::CreateMessageTest;
    pub use tests::controller_test::ControllerTest;
    pub use tests::qbft_message::QbftMessageTest;
    pub use tests::round_robin::RoundRobinTest;
    
    // Existing enum definitions
    #[derive(Eq, PartialEq, Hash, Debug)]
    pub(crate) enum QbftSpecTestType {
        QbftMessage,
        CreateMessage,
        RoundRobin,
        Controller,
    }
    
    impl std::fmt::Display for QbftSpecTestType {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            match self {
                QbftSpecTestType::QbftMessage => write!(f, "MsgSpecTest"),
                QbftSpecTestType::CreateMessage => write!(f, "CreateMsgSpecTest"),
                QbftSpecTestType::RoundRobin => write!(f, "RoundRobinSpecTest"),
                QbftSpecTestType::Controller => write!(f, "ControllerSpecTest"),
            }
        }
    }
    ```
  - **Updates lib.rs** to register refactored test modules
  - **Integration**: Ensures all refactored modules are properly exported and registered
  - **Context**: Maintains existing test registration patterns while using refactored code

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