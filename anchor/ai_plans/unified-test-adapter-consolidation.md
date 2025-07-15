# Unified Test Adapter Framework Consolidation Plan

## Executive Summary

### Problem Statement
The current QBFT testing infrastructure suffers from significant duplication and fragmentation across multiple test adapters (`QbftTestAdapter`, `EnhancedTestAdapter`, `SpecQbft`), extensive unused mock infrastructure, and stub implementations that provide no value. Analysis reveals that while comprehensive testing frameworks were built, only a small subset is actually used, creating maintenance overhead and cognitive complexity without corresponding benefit.

### Proposed Solution
Consolidate all testing functionality into a single, professional `UnifiedTestAdapter` that combines the essential features of existing adapters while removing unused infrastructure. This consolidation will eliminate over 500 lines of duplicate code and 3 separate adapter implementations, while maintaining full compatibility with Go QBFT spec tests and all current functionality.

### Technical Approach
1. **Create UnifiedTestAdapter**: Merge QbftTestAdapter + SpecQbft functionality into single configurable adapter
2. **Remove Unused Infrastructure**: Delete EnhancedTestAdapter, mock infrastructure, and stub test implementations
3. **Streamline Configuration**: Replace multiple config types with single unified configuration
4. **Maintain Spec Compliance**: Preserve Go QBFT compatibility and JSON test integration
5. **Clean API Design**: Provide simple, professional interface for all testing needs

### Data Flow
```
JSON Test Files → UnifiedTestAdapter → Core QBFT Logic → Message Creation/Validation → Test Results
                       ↓
              Unified Configuration
            (Committee, Signing, Scenarios)
```

### Expected Outcomes
- **Reduce complexity**: From 3 test adapters to 1 unified adapter
- **Eliminate dead code**: Remove 500+ lines of unused mock infrastructure and stubs
- **Improve maintainability**: Single configuration, API, and implementation to maintain
- **Preserve functionality**: Maintain all current test capabilities and Go spec compatibility
- **Professional codebase**: Clean, focused testing framework with clear purpose

## Goals & Objectives

### Primary Goals
- **Code Consolidation**: Reduce from 3 test adapters (QbftTestAdapter, EnhancedTestAdapter, SpecQbft) to 1 unified adapter
- **Remove Dead Code**: Eliminate unused mock infrastructure, stub implementations, and theoretical features
- **Maintain Compatibility**: Preserve 100% Go QBFT spec test compatibility and all current test functionality

### Secondary Objectives
- **Improve Developer Experience**: Provide single, clean API for all QBFT testing needs
- **Reduce Maintenance Burden**: Single codebase to maintain instead of fragmented implementations
- **Professional Quality**: Create well-designed, documented testing framework

## Solution Overview

### Approach
Analyze current usage patterns to identify essential vs unused functionality, then create a single `UnifiedTestAdapter` that combines the genuinely used features while eliminating theoretical capabilities that add complexity without value. Focus on proven functionality rather than comprehensive but unused frameworks.

### Key Components
1. **UnifiedTestAdapter**: Single adapter combining QbftTestAdapter + SpecQbft functionality
2. **Unified Configuration**: Single config type replacing multiple overlapping configurations
3. **Core Test Types**: Preserve working test types (CreateMessage, QbftMessage, RoundRobin)
4. **Cleanup Operations**: Remove unused infrastructure and stub implementations

### Architecture Diagram
```
Current State:
QbftTestAdapter → SpecQbft Wrapper → Tests
EnhancedTestAdapter (unused) → Mock Infrastructure (unused)
Stub Test Types (unused)

Unified State:
JSON Test Files → UnifiedTestAdapter → Core QBFT Logic → Test Results
                       ↓
                Unified Configuration
```

### Expected Outcomes
- **Single test adapter**: Replace 3 adapters with 1 configurable implementation
- **Reduced codebase**: Eliminate 500+ lines of unused infrastructure and stubs
- **Maintained functionality**: All current test capabilities preserved
- **Improved clarity**: Clear, focused testing framework with obvious purpose

## Implementation Tasks

### CRITICAL IMPLEMENTATION RULES
1. **PRESERVE FUNCTIONALITY**: All current working tests must continue to work exactly as before
2. **REMOVE DEAD CODE**: Aggressively eliminate unused infrastructure and stub implementations
3. **MAINTAIN COMPATIBILITY**: Go QBFT spec test compatibility must be preserved
4. **SINGLE SOURCE OF TRUTH**: One adapter, one configuration, one API for all testing

### Visual Dependency Tree
```
spec_tests/src/
├── qbft/
│   ├── unified_test_adapter.rs (Task #1: Create unified adapter)
│   ├── test_adapter.rs (Task #3: Remove after migration)
│   ├── enhanced_test_adapter.rs (Task #3: Remove - unused)
│   ├── mock_infrastructure.rs (Task #3: Remove - unused)
│   ├── test_verification.rs (Task #3: Remove - unused)
│   ├── qbft_message.rs (Task #2: Update to use unified adapter)
│   ├── create_message.rs (Task #2: Update to use unified adapter)
│   ├── round_robin.rs (Task #2: Update to use unified adapter)
│   ├── timeout.rs (Task #3: Remove - stub implementation)
│   ├── controller.rs (Task #3: Remove - stub implementation)
│   ├── message_processing.rs (Task #3: Remove - stub implementation)
│   └── mod.rs (Task #4: Update exports and remove deprecated types)
│
├── types/
│   ├── test_framework.rs (Task #3: Remove - unused types)
│   └── qbft_message.rs (Task #2: Update for unified adapter)
│
├── utils/
│   └── spec_test_runner.rs (Task #3: Remove - unused)
│
└── lib.rs (Task #4: Update test registration)
```

### Execution Plan

#### Group A: Core Consolidation (Execute sequentially for safety)
- [x] **Task #1**: Create UnifiedTestAdapter consolidating essential functionality
  - **Folder**: `spec_tests/src/qbft/`
  - **File**: `unified_test_adapter.rs`
  - **Purpose**: Replace QbftTestAdapter + SpecQbft with single, configurable adapter
  - **Core Implementation Requirements**:
    ```rust
    use std::collections::HashMap;
    use indexmap::IndexSet;
    use openssl::pkey::{PKey, Private};
    use qbft::{Qbft, DefaultLeaderFunction, TestConfig};
    use ssv_types::{
        OperatorId, Round, CommitteeId,
        message::SignedSSVMessage,
        consensus::{BeaconVote, QbftMessageType, UnsignedSSVMessage},
        msgid::MessageId,
    };
    use types::{Hash256, Slot};
    
    /// Unified test adapter combining QbftTestAdapter + SpecQbft functionality
    pub struct UnifiedTestAdapter {
        // Core QBFT instance
        qbft: Qbft<DefaultLeaderFunction, BeaconVote, TestMessageSender>,
        
        // Test configuration
        committee: IndexSet<OperatorId>,
        identifier: MessageId,
        config: TestConfig,
        
        // Signing infrastructure
        signing_key: Option<PKey<Private>>,
        
        // Test state tracking
        processed_messages: Vec<SignedSSVMessage>,
        current_round: Round,
        current_height: u64,
    }
    
    impl UnifiedTestAdapter {
        /// Create new unified test adapter
        pub fn new(
            committee: IndexSet<OperatorId>, 
            identifier: MessageId, 
            config: TestConfig
        ) -> Result<Self, qbft::TestError> {
            // Initialize core QBFT instance
            let qbft = Qbft::new(
                committee.clone(),
                identifier.clone(),
                TestMessageSender::new(),
                config.clone(),
            )?;
            
            Ok(Self {
                qbft,
                committee,
                identifier,
                config,
                signing_key: None,
                processed_messages: Vec::new(),
                current_round: Round::from(1),
                current_height: 0,
            })
        }
        
        /// Set signing key for message signing
        pub fn set_signing_key(&mut self, key: PKey<Private>) {
            self.signing_key = Some(key);
        }
        
        /// Setup test scenario (from QbftTestAdapter)
        pub fn setup_test_scenario(&mut self, scenario: TestScenario) -> Result<(), qbft::TestError> {
            // Apply round change and prepare justifications from scenario
            if let Some(round_changes) = scenario.round_change_justifications {
                for rc_msg in round_changes {
                    self.process_message(rc_msg)?;
                }
            }
            
            if let Some(prepares) = scenario.prepare_justifications {
                for prep_msg in prepares {
                    self.process_message(prep_msg)?;
                }
            }
            
            Ok(())
        }
        
        /// Create proposal message
        pub fn create_proposal(
            &mut self,
            data_hash: Hash256,
            round: Option<Round>,
        ) -> Result<SignedSSVMessage, qbft::TestError> {
            let unsigned = self.qbft.new_unsigned_message_spec(
                QbftMessageType::Proposal,
                data_hash,
                vec![], // No justifications for proposal
                vec![], // No justifications for proposal
                round,
            );
            self.sign_message(unsigned)
        }
        
        /// Create prepare message
        pub fn create_prepare(
            &mut self,
            data_hash: Hash256,
            round: Option<Round>,
        ) -> Result<SignedSSVMessage, qbft::TestError> {
            let unsigned = self.qbft.new_unsigned_message_spec(
                QbftMessageType::Prepare,
                data_hash,
                vec![], // No justifications for prepare
                vec![], // No justifications for prepare
                round,
            );
            self.sign_message(unsigned)
        }
        
        /// Create commit message
        pub fn create_commit(
            &mut self,
            data_hash: Hash256,
            round: Option<Round>,
        ) -> Result<SignedSSVMessage, qbft::TestError> {
            let unsigned = self.qbft.new_unsigned_message_spec(
                QbftMessageType::Commit,
                data_hash,
                vec![], // No justifications for commit
                vec![], // No justifications for commit
                round,
            );
            self.sign_message(unsigned)
        }
        
        /// Create round change message
        pub fn create_round_change(
            &mut self,
            state_value: Option<Vec<u8>>,
            target_round: Option<Round>,
        ) -> Result<SignedSSVMessage, qbft::TestError> {
            // Get round change and prepare justifications from current state
            let (rc_justifications, prep_justifications) = self.qbft.get_justifications_for_round_change();
            
            let unsigned = self.qbft.new_unsigned_message_spec(
                QbftMessageType::RoundChange,
                Hash256::default(), // Will be set based on state_value
                rc_justifications,
                prep_justifications,
                target_round,
            );
            
            // Handle state value if provided
            if let Some(state_bytes) = state_value {
                unsigned.unsigned_message.full_data = state_bytes;
            }
            
            self.sign_message(unsigned)
        }
        
        /// Create message with comprehensive parameters (from SpecQbft)
        pub fn create_message(
            &mut self,
            message_type: QbftMessageType,
            data_hash: Hash256,
            round: Option<Round>,
            state_value: Option<Vec<u8>>,
            round_change_justifications: Vec<SignedSSVMessage>,
            prepare_justifications: Vec<SignedSSVMessage>,
        ) -> Result<SignedSSVMessage, qbft::TestError> {
            // Handle round change logic with state value
            let effective_data_hash = if message_type == QbftMessageType::RoundChange {
                if let Some(ref state_value_bytes) = state_value {
                    use sha2::{Sha256, Digest};
                    Hash256::from_slice(&Sha256::digest(state_value_bytes))
                } else {
                    Hash256::default()
                }
            } else {
                data_hash
            };
            
            let mut unsigned = self.qbft.new_unsigned_message_spec(
                message_type,
                effective_data_hash,
                round_change_justifications,
                prepare_justifications,
                round,
            );
            
            // Handle round change state value
            if message_type == QbftMessageType::RoundChange {
                if let Some(state_value_bytes) = state_value {
                    unsigned.unsigned_message.full_data = state_value_bytes;
                }
            }
            
            self.sign_message(unsigned)
        }
        
        /// Verify message root (from SpecQbft)
        pub fn verify_root(&self, msg: SignedSSVMessage, root: Hash256) -> bool {
            use tree_hash::TreeHash;
            msg.tree_hash_root() == root
        }
        
        /// Process message through QBFT
        pub fn process_message(&mut self, msg: SignedSSVMessage) -> Result<qbft::ProcessingResult, qbft::TestError> {
            let result = self.qbft.process_message(msg.clone())?;
            self.processed_messages.push(msg);
            Ok(result)
        }
        
        /// Get current round
        pub fn get_current_round(&self) -> Round {
            self.current_round
        }
        
        /// Get QBFT state
        pub fn get_state(&self) -> qbft::QbftState {
            self.qbft.get_state()
        }
        
        /// Sign message using deterministic RSA signing
        fn sign_message(&self, unsigned: UnsignedWrappedQbftMessage) -> Result<SignedSSVMessage, qbft::TestError> {
            if let Some(ref key) = self.signing_key {
                // Use deterministic RSA signing for spec test compatibility
                use openssl::hash::MessageDigest;
                use openssl::sign::Signer;
                
                let data = unsigned.ssz_encode()?;
                let mut signer = Signer::new(MessageDigest::sha256(), key)?;
                let signature = signer.sign_oneshot(&data)?;
                
                // Create signed message with single signature
                SignedSSVMessage::new_from_vecs(
                    vec![signature],
                    vec![self.get_operator_id()],
                    unsigned.unsigned_message.message,
                    unsigned.unsigned_message.full_data,
                ).map_err(|e| qbft::TestError::SigningFailed(format!("Failed to create signed message: {}", e)))
            } else {
                Err(qbft::TestError::SigningFailed("No signing key set".to_string()))
            }
        }
        
        /// Get operator ID for current test
        fn get_operator_id(&self) -> OperatorId {
            self.committee.iter().next().copied().unwrap_or(OperatorId::from(1))
        }
    }
    
    /// Test scenario configuration (preserved from QbftTestAdapter)
    #[derive(Clone)]
    pub struct TestScenario {
        pub round_change_justifications: Option<Vec<SignedSSVMessage>>,
        pub prepare_justifications: Option<Vec<SignedSSVMessage>>,
    }
    ```
  - **Integration Points**:
    - Must work with existing CreateMessageTest, QbftMessageTest, RoundRobinTest
    - Maintain compatibility with JSON test file parsing
    - Preserve Go QBFT spec test compatibility
    - Support all current message creation and validation patterns
  - **Key Features**:
    - All message creation methods from QbftTestAdapter
    - Comprehensive create_message() method from SpecQbft
    - Root verification capabilities
    - Test scenario setup and state management
    - Deterministic RSA signing for spec compatibility

#### Group B: Test Migration (Execute in parallel after Group A)
- [x] **Task #2**: Update working test types to use UnifiedTestAdapter
  - **Files**: `qbft_message.rs`, `create_message.rs`, `round_robin.rs`, `types/qbft_message.rs`
  - **Purpose**: Migrate all working test implementations to use UnifiedTestAdapter
  - **QbftMessageTest Migration**:
    ```rust
    // Update QbftMessageTest to use UnifiedTestAdapter
    impl SpecTest for QbftMessageTest {
        fn run(&mut self) -> Result<(), String> {
            for (i, test_message) in self.messages.iter().enumerate() {
                // Create unified adapter for this test
                let mut adapter = self.create_unified_adapter()?;
                
                // Process message through unified adapter
                match self.validate_message_with_adapter(&mut adapter, test_message) {
                    Ok(_) => {
                        if !self.expected_error.is_empty() {
                            return Err(format!("Test {} should have failed but succeeded", i));
                        }
                    }
                    Err(error) => {
                        if self.expected_error.is_empty() || !error.contains(&self.expected_error) {
                            return Err(format!("Test {}: {}", i, error));
                        }
                    }
                }
            }
            Ok(())
        }
        
        fn create_unified_adapter(&self) -> Result<UnifiedTestAdapter, String> {
            // Extract committee from test messages
            let committee = self.extract_committee_from_messages();
            
            // Create test config
            let config = TestConfig {
                committee_size: committee.len(),
                quorum_threshold: (committee.len() * 2 / 3) + 1,
                max_rounds: 100,
                instance_height: 0,
            };
            
            // Create adapter
            UnifiedTestAdapter::new(committee, self.create_message_id(), config)
                .map_err(|e| format!("Failed to create adapter: {}", e))
        }
    }
    ```
  - **CreateMessageTest Migration**:
    ```rust
    // Update CreateMessageTest to use UnifiedTestAdapter
    impl SpecTest for CreateMessageTest {
        fn run(&mut self) -> Result<(), String> {
            // Create unified adapter
            let mut adapter = self.create_unified_adapter()?;
            
            // Setup test scenario if needed
            if let Some(scenario) = self.create_test_scenario() {
                adapter.setup_test_scenario(scenario)
                    .map_err(|e| format!("Scenario setup failed: {}", e))?;
            }
            
            // Create message using unified adapter
            let result = adapter.create_message(
                self.message_type,
                self.data_hash,
                self.round,
                self.state_value.clone(),
                self.round_change_justifications.clone(),
                self.prepare_justifications.clone(),
            );
            
            // Verify result
            match result {
                Ok(message) => {
                    if !self.expected_error.is_empty() {
                        return Err("Test should have failed but succeeded".to_string());
                    }
                    
                    // Verify message root if expected
                    if let Some(expected_root) = self.expected_root {
                        if !adapter.verify_root(message, expected_root) {
                            return Err("Message root verification failed".to_string());
                        }
                    }
                }
                Err(error) => {
                    if self.expected_error.is_empty() || !format!("{}", error).contains(&self.expected_error) {
                        return Err(format!("Unexpected error: {}", error));
                    }
                }
            }
            
            Ok(())
        }
    }
    ```
  - **RoundRobinTest Migration**:
    ```rust
    // Update RoundRobinTest to use UnifiedTestAdapter
    impl SpecTest for RoundRobinTest {
        fn run(&mut self) -> Result<(), String> {
            // Create unified adapter
            let adapter = self.create_unified_adapter()?;
            
            // Test leader function through adapter's committee configuration
            let leader_function = DefaultLeaderFunction::new();
            
            for test_case in &self.test_cases {
                let leader = leader_function.get_leader(
                    &adapter.committee,
                    test_case.round,
                    test_case.height,
                );
                
                if leader != test_case.expected_leader {
                    return Err(format!(
                        "Round {} height {}: expected leader {}, got {}",
                        test_case.round, test_case.height, test_case.expected_leader, leader
                    ));
                }
            }
            
            Ok(())
        }
    }
    ```
  - **Success Criteria**:
    - All existing working tests pass with UnifiedTestAdapter
    - No functionality is lost in migration
    - Test execution time remains comparable
    - Go QBFT spec compatibility is maintained

#### Group C: Cleanup and Removal (Execute in parallel after Group B)
- [x] **Task #3**: Remove unused infrastructure and stub implementations
  - **Files to Remove Completely**:
    ```rust
    // Remove these files entirely:
    - `enhanced_test_adapter.rs` (324 lines, unused)
    - `mock_infrastructure.rs` (481 lines, unused)
    - `test_verification.rs` (438 lines, unused)
    - `timeout.rs` (stub implementation)
    - `controller.rs` (stub implementation)
    - `message_processing.rs` (stub implementation)
    - `types/test_framework.rs` (unused types)
    - `utils/spec_test_runner.rs` (unused)
    ```
  - **Removal Verification**:
    - Verify no imports reference removed files
    - Ensure no tests depend on removed functionality
    - Remove related exports from mod.rs files
    - Clean up any unused dependencies in Cargo.toml
  - **Remove test_adapter.rs after migration**:
    - Only remove after UnifiedTestAdapter is fully functional
    - Verify all tests pass with unified adapter first
    - Update all references to use UnifiedTestAdapter
  - **Expected Impact**:
    - Remove 1,200+ lines of unused code
    - Eliminate maintenance burden of unused infrastructure
    - Simplify codebase and reduce cognitive load

#### Group D: Final Integration (Execute after Groups B and C)
- [x] **Task #4**: Update module structure and test registration
  - **Files**: `qbft/mod.rs`, `lib.rs`
  - **Module Updates**:
    ```rust
    // Update qbft/mod.rs to export only unified adapter
    mod create_message;
    mod qbft_message;
    mod round_robin;
    mod unified_test_adapter;
    
    pub use create_message::CreateMessageTest;
    pub use qbft_message::QbftMessageTest;
    pub use round_robin::RoundRobinTest;
    pub use unified_test_adapter::{UnifiedTestAdapter, TestScenario};
    
    // Remove all deprecated exports:
    // - enhanced_test_adapter
    // - mock_infrastructure
    // - test_verification
    // - test_adapter
    // - timeout, controller, message_processing
    ```
  - **Test Registration Updates**:
    ```rust
    // Update lib.rs to remove stub test registrations
    test_loaders! {
        qbft_tests: [
            // Keep working tests
            (QbftSpecTestType::CreateMessage, "CreateMsgSpecTest", CreateMessageTest),
            (QbftSpecTestType::QbftMessage, "MsgSpecTest", QbftMessageTest),
            (QbftSpecTestType::RoundRobin, "RoundRobinSpecTest", RoundRobinTest),
            
            // Remove stub tests:
            // - TimeoutTest (stub)
            // - ControllerTest (stub)
            // - MessageProcessingTest (stub)
        ],
        // ... other test types
    }
    ```
  - **Validation**:
    - All tests compile and run successfully
    - No broken imports or missing exports
    - Test registration works correctly
    - Documentation is updated for new structure

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
- Execute tasks sequentially in Group A for safety, parallel execution in other groups

### Progress Tracking
The checkboxes above represent the authoritative status of each task. Keep them updated as you work.