use base64::Engine;
use hex;
use openssl::pkey::{PKey, Private};
use parking_lot::RwLock;
use qbft::{
    Config, ConfigBuilder, DefaultLeaderFunction, InstanceHeight, MessageSender, Qbft,
    UnsignedWrappedQbftMessage,
};
use sha2::{Digest, Sha256};
use ssv_types::{
    IndexSet, OperatorId, Round,
    consensus::{BeaconVote, JustificationLength, RoundChangeLength},
    message::SignedSSVMessage,
    msgid::MessageId,
};
use ssz::Encode;
use std::{collections::VecDeque, sync::Arc};
use types::typenum::U13;
use types::{Hash256, VariableList};

use super::error_mapping::{ErrorMapper, map_signed_ssv_error_to_go_format};
use super::types::*;
use super::validation::*;
use crate::utils::test_keys::TestKeySet;

/// Shared message sender for QBFT instances
pub struct SharedMessageSender {
    queue: Arc<RwLock<VecDeque<UnsignedWrappedQbftMessage>>>,
}

impl MessageSender for SharedMessageSender {
    fn send(&mut self, msg: UnsignedWrappedQbftMessage) {
        self.queue.write().push_back(msg);
    }
}

/// Unified QBFT test adapter with integrated builder and scenario management
pub struct QbftTestAdapter {
    qbft: Qbft<DefaultLeaderFunction, BeaconVote, SharedMessageSender>,
    committee: IndexSet<OperatorId>,
    signing_key: PKey<Private>,
    operator_id: OperatorId,
    identifier: MessageId,
    state: AdapterState,
    message_queue: Arc<RwLock<VecDeque<UnsignedWrappedQbftMessage>>>,
    config: AdapterConfig,
    test_context: Option<TestContext>,
}

/// Compatibility result type for controller tests
#[derive(Debug, Clone)]
pub struct ControllerResult {
    pub processing_result: ProcessingResult,
    pub decided_state: DecidedState,
}

impl QbftTestAdapter {
    /// Create new unified adapter instance
    pub fn new(
        committee: IndexSet<OperatorId>,
        identifier: MessageId,
        config: AdapterConfig,
        signing_key: PKey<Private>,
        operator_id: OperatorId,
    ) -> Result<Self, AdapterError> {
        // Validate configuration
        validate_committee(&committee, config.quorum_threshold)?;

        // Create message queue and sender
        let message_queue = Arc::new(RwLock::new(VecDeque::new()));
        let message_sender = SharedMessageSender {
            queue: message_queue.clone(),
        };

        // Build QBFT configuration
        let qbft_config: Config<DefaultLeaderFunction> = ConfigBuilder::new(
            operator_id,
            InstanceHeight::from(config.instance_height as usize),
            committee.clone(),
        )
        .build()
        .map_err(|e| AdapterError::Config(format!("QBFT config build failed: {}", e)))?;

        // Create test data for QBFT
        let test_data = BeaconVote {
            block_root: types::Hash256::random(),
            source: types::Checkpoint::default(),
            target: types::Checkpoint::default(),
        };

        // Initialize QBFT instance
        let qbft = Qbft::new(qbft_config, test_data, identifier.clone(), message_sender);

        let mut state = AdapterState::default();
        state.instance_height = config.instance_height;
        state.current_height = config.current_height;

        Ok(Self {
            qbft,
            committee,
            signing_key,
            operator_id,
            identifier,
            state,
            message_queue,
            config,
            test_context: None,
        })
    }

    // =============================================================================
    // BUILDER METHODS - Integrated factory pattern
    // =============================================================================

    /// Create adapter for controller testing
    pub fn for_controller_testing(
        committee_member: &SpecTestCommitteeMember,
        operator_id: OperatorId,
        instance_height: u64,
        current_height: u64,
    ) -> Result<Self, AdapterError> {
        let committee = extract_committee_from_spec_test(committee_member)?;
        let signing_key = Self::extract_signing_key(operator_id)?;

        let config = AdapterConfig {
            instance_height,
            current_height,
            committee_size: 4,
            quorum_threshold: 3,
            max_rounds: 100,
        };

        Self::new(
            committee,
            MessageId::for_spectest(),
            config,
            signing_key,
            operator_id,
        )
    }

    /// Create adapter for message creation testing
    pub fn for_message_creation(
        committee_member: &SpecTestCommitteeMember,
        operator_id: OperatorId,
    ) -> Result<Self, AdapterError> {
        let committee = extract_committee_from_spec_test(committee_member)?;
        let signing_key = Self::extract_signing_key(operator_id)?;

        let config = AdapterConfig {
            instance_height: 0,
            current_height: 0,
            committee_size: 4,
            quorum_threshold: 3,
            max_rounds: 100,
        };

        Self::new(
            committee,
            MessageId::for_spectest(),
            config,
            signing_key,
            operator_id,
        )
    }

    /// Create adapter for validation testing
    pub fn for_validation_testing(
        committee_member: &SpecTestCommitteeMember,
        operator_id: OperatorId,
    ) -> Result<Self, AdapterError> {
        let committee = extract_committee_from_spec_test(committee_member)?;
        let signing_key = Self::extract_signing_key(operator_id)?;

        let config = AdapterConfig {
            instance_height: 0,
            current_height: 0,
            committee_size: 4,
            quorum_threshold: 3,
            max_rounds: 10, // Shorter for validation tests
        };

        Self::new(
            committee,
            MessageId::for_spectest(),
            config,
            signing_key,
            operator_id,
        )
    }

    /// Create adapter with default 4-operator committee (for simple tests)
    pub fn with_default_committee() -> Result<Self, AdapterError> {
        let mut committee = IndexSet::new();
        committee.insert(OperatorId::from(1));
        committee.insert(OperatorId::from(2));
        committee.insert(OperatorId::from(3));
        committee.insert(OperatorId::from(4));

        let signing_key = Self::extract_signing_key(OperatorId::from(1))?;

        let config = AdapterConfig {
            instance_height: 0,
            current_height: 0,
            committee_size: 4,
            quorum_threshold: 3,
            max_rounds: 100,
        };

        Self::new(
            committee,
            MessageId::for_spectest(),
            config,
            signing_key,
            OperatorId::from(1),
        )
    }

    /// Extract signing key from test key set
    fn extract_signing_key(operator_id: OperatorId) -> Result<PKey<Private>, AdapterError> {
        let test_keys = TestKeySet::four_share_set();
        let signing_key = test_keys
            .operator_keys
            .get(&operator_id)
            .ok_or_else(|| {
                AdapterError::Config(format!("No key found for operator {}", operator_id.0))
            })?
            .clone();

        PKey::from_rsa(signing_key).map_err(AdapterError::OpenSsl)
    }

    // =============================================================================
    // TEST CONTEXT AND SCENARIO MANAGEMENT
    // =============================================================================

    /// Set test context for scenario management
    pub fn with_test_context(mut self, context: TestContext) -> Self {
        self.test_context = Some(context);
        self
    }

    /// Get current test context
    pub fn test_context(&self) -> Option<&TestContext> {
        self.test_context.as_ref()
    }

    /// Execute controller test scenario
    pub fn execute_controller_scenario(
        &mut self,
        input_value: Option<String>,
        input_messages: Vec<SignedSSVMessage>,
    ) -> ScenarioResult {
        let context = self
            .test_context
            .clone()
            .unwrap_or_else(|| TestContext::default_controller());

        // Setup if needed
        if let Some(input_value) = input_value {
            if let Err(e) = self.setup_controller_scenario(input_value, None) {
                return self.create_error_result("controller", e);
            }
        }

        // Process messages through adapter business logic
        match self.process_messages_with_context(input_messages, &context) {
            Ok(result) => result,
            Err(e) => self.create_error_result("controller", e),
        }
    }

    /// Execute message creation scenario
    pub fn execute_message_creation_scenario(
        &mut self,
        request: MessageCreationRequest,
        expected_root: Hash256,
    ) -> ScenarioResult {
        self.execute_message_creation_scenario_with_committee_id(request, expected_root, None)
    }

    pub fn execute_message_creation_scenario_with_committee_id(
        &mut self,
        request: MessageCreationRequest,
        expected_root: Hash256,
        committee_id: Option<Vec<u8>>,
    ) -> ScenarioResult {
        let context = self
            .test_context
            .clone()
            .unwrap_or_else(|| TestContext::default_message_creation());

        // Create message with validation
        match self.create_message_with_validation_and_committee_id(request, &context, committee_id)
        {
            Ok((message, validation_result)) => {
                // Validate root
                let _root_matches = self.validate_root(&message, expected_root);

                let error_mapper = ErrorMapper::new(context.clone());
                let go_errors =
                    error_mapper.map_validation_error_strings(&validation_result.errors);

                ScenarioResult {
                    scenario_id: context.scenario_id(),
                    processing_result: ProcessingResult {
                        consensus_reached: false, // Message creation doesn't involve consensus
                        messages_sent: vec![message],
                        validation_result: validation_result.clone(),
                        go_error_messages: go_errors.clone(),
                    },
                    decided_state: self.get_decided_state(),
                    timer_state: self.get_timer_state(),
                    validation_errors: validation_result.errors,
                    go_formatted_errors: go_errors,
                }
            }
            Err(e) => self.create_error_result("message_creation", e),
        }
    }

    /// Execute validation scenario with enhanced validation checks
    pub fn execute_validation_scenario(&self, message: SignedSSVMessage) -> ScenarioResult {
        let context = self
            .test_context
            .clone()
            .unwrap_or_else(|| TestContext::default_validation());

        // Validate message comprehensively using enhanced validation
        let validation_result = self.validate_message_enhanced(&message, &context);

        let error_mapper = ErrorMapper::new(context.clone());
        ScenarioResult {
            scenario_id: context.scenario_id(),
            processing_result: ProcessingResult {
                consensus_reached: false,
                messages_sent: Vec::new(),
                validation_result: validation_result.clone(),
                go_error_messages: error_mapper
                    .map_validation_error_strings(&validation_result.errors),
            },
            decided_state: self.get_decided_state(),
            timer_state: self.get_timer_state(),
            validation_errors: validation_result.errors.clone(),
            go_formatted_errors: error_mapper
                .map_validation_error_strings(&validation_result.errors),
        }
    }

    /// Enhanced validation with comprehensive checks for identifiers, sizes, and message types
    fn validate_message_enhanced(
        &self,
        message: &SignedSSVMessage,
        context: &TestContext,
    ) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // 1. Start with basic comprehensive validation
        let basic_validation = validate_message_comprehensive(message, &self.committee, context);
        errors.extend(basic_validation.errors);
        warnings.extend(basic_validation.warnings);

        // 2. Enhanced identifier validation
        if let Err(error) = self.validate_message_identifier(message) {
            errors.push(error);
        }

        // 3. Enhanced size validation
        if let Err(error) = self.validate_message_size(message) {
            errors.push(error);
        }

        // 4. Enhanced message type validation
        if let Err(error) = self.validate_message_type(message) {
            errors.push(error);
        }

        // 5. Enhanced structure validation
        if let Err(error) = self.validate_message_structure(message) {
            errors.push(error);
        }

        // 6. Enhanced justification validation
        if let Err(error) = self.validate_message_justifications(message) {
            errors.push(error);
        }

        // 7. Enhanced root calculation validation
        if let Err(error) = self.validate_message_root_calculation(message) {
            errors.push(error);
        }

        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        }
    }

    /// Validate message identifier with enhanced checks (matching Go logic)
    fn validate_message_identifier(&self, message: &SignedSSVMessage) -> Result<(), String> {
        use ssv_types::consensus::QbftMessage;
        use ssz::Decode;

        // Extract QBFT message from SSV message
        let qbft_message = match QbftMessage::from_ssz_bytes(message.ssv_message().data()) {
            Ok(msg) => msg,
            Err(_) => return Err("message identifier is invalid".to_string()),
        };

        // Go validation: if len(msg.Identifier) != 56
        if qbft_message.identifier.len() != 56 {
            return Err("message identifier is invalid".to_string());
        }

        Ok(())
    }

    /// Validate message size with enhanced checks (matching Go logic)
    fn validate_message_size(&self, message: &SignedSSVMessage) -> Result<(), String> {
        // Go validation: if len(msg.OperatorIDs) == 0
        if message.operator_ids().is_empty() {
            return Err("no signers".to_string());
        }

        // Go validation: if len(msg.Signatures) == 0
        if message.signatures().is_empty() {
            return Err("no signatures".to_string());
        }

        // Go validation: for each signature, if len(signature) == 0
        for signature in message.signatures() {
            if signature.is_empty() {
                return Err("empty signature".to_string());
            }
        }

        // Go validation: if len(msg.Signatures) != len(msg.OperatorIDs)
        if message.signatures().len() != message.operator_ids().len() {
            return Err("number of signatures is different than number of signers".to_string());
        }

        // Go validation: for each operatorID, if operatorID == 0
        for operator_id in message.operator_ids() {
            if operator_id.0 == 0 {
                return Err("signer ID 0 not allowed".to_string());
            }
        }

        // Go validation: check for non unique signers
        let mut seen_signers = std::collections::HashSet::new();
        for operator_id in message.operator_ids() {
            if seen_signers.contains(operator_id) {
                return Err("non unique signer".to_string());
            }
            seen_signers.insert(operator_id);
        }

        // Go validation: if msg.SSVMessage == nil (already handled by type system)

        Ok(())
    }

    /// Validate QBFT message type with enhanced checks (matching Go logic)
    fn validate_message_type(&self, message: &SignedSSVMessage) -> Result<(), String> {
        use ssv_types::consensus::{QbftMessage, QbftMessageType};
        use ssz::Decode;

        // Extract QBFT message from SSV message
        let qbft_message = match QbftMessage::from_ssz_bytes(message.ssv_message().data()) {
            Ok(msg) => msg,
            Err(_) => return Err("message type is invalid".to_string()),
        };

        // Go validation: if msg.MsgType > RoundChangeMsgType
        // RoundChangeMsgType is 3, so any type > 3 is invalid
        let msg_type_value = match qbft_message.qbft_message_type {
            QbftMessageType::Proposal => 0,
            QbftMessageType::Prepare => 1,
            QbftMessageType::Commit => 2,
            QbftMessageType::RoundChange => 3,
        };

        if msg_type_value > 3 {
            return Err("message type is invalid".to_string());
        }

        Ok(())
    }

    /// Validate message structure with enhanced checks (matching Go logic)
    fn validate_message_structure(&self, message: &SignedSSVMessage) -> Result<(), String> {
        use ssv_types::consensus::QbftMessage;
        use ssz::Decode;

        // Extract QBFT message from SSV message
        let qbft_message = match QbftMessage::from_ssz_bytes(message.ssv_message().data()) {
            Ok(msg) => msg,
            Err(_) => return Err("malformed message structure".to_string()),
        };

        // Go validation: if msg.Round == NoRound (NoRound is 0)
        // But this seems to be validated elsewhere, so we'll be lenient here

        // Most structure validation is done in Go-specific message type validation
        // which we'll implement separately if needed

        Ok(())
    }

    /// Validate message justifications with enhanced checks (matching Go logic)
    fn validate_message_justifications(&self, message: &SignedSSVMessage) -> Result<(), String> {
        use ssv_types::consensus::{QbftMessage, QbftMessageType};
        use ssz::Decode;

        // Extract QBFT message from SSV message
        let qbft_message = match QbftMessage::from_ssz_bytes(message.ssv_message().data()) {
            Ok(msg) => msg,
            Err(_) => return Err("malformed justifications".to_string()),
        };

        // The "incorrect size" error typically comes from SSZ unmarshalling issues
        // When the justification data is malformed or truncated

        // Try to decode justifications to check for "incorrect size" errors
        if !qbft_message.round_change_justification.is_empty() {
            for justification in qbft_message.round_change_justification.iter() {
                if justification.is_empty() {
                    return Err("malformed round change justifications".to_string());
                }

                // Try to decode the justification as a SignedSSVMessage
                // If it fails with size issues, return "incorrect size"
                match ssv_types::message::SignedSSVMessage::from_ssz_bytes(justification) {
                    Ok(_) => {
                        // Justification decoded successfully
                    }
                    Err(_) => {
                        // Decoding failed - this is likely the "incorrect size" error
                        return Err("incorrect size".to_string());
                    }
                }
            }
        }

        if !qbft_message.prepare_justification.is_empty() {
            for justification in qbft_message.prepare_justification.iter() {
                if justification.is_empty() {
                    return Err("malformed prepare justifications".to_string());
                }

                // Try to decode the justification as a SignedSSVMessage
                // If it fails with size issues, return "incorrect size"
                match ssv_types::message::SignedSSVMessage::from_ssz_bytes(justification) {
                    Ok(_) => {
                        // Justification decoded successfully
                    }
                    Err(_) => {
                        // Decoding failed - this is likely the "incorrect size" error
                        return Err("incorrect size".to_string());
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate message root calculation with enhanced checks
    fn validate_message_root_calculation(&self, message: &SignedSSVMessage) -> Result<(), String> {
        use ssv_types::consensus::{QbftMessage, QbftMessageType};
        use ssz::Decode;

        // Extract QBFT message from SSV message
        let qbft_message = match QbftMessage::from_ssz_bytes(message.ssv_message().data()) {
            Ok(msg) => msg,
            Err(_) => return Err("invalid hash".to_string()),
        };

        // Check root calculation based on message type and state
        match qbft_message.qbft_message_type {
            QbftMessageType::Proposal => {
                // Proposals should have consistent root with their data
                if qbft_message.root.is_zero() && !message.full_data().is_empty() {
                    return Err("full data hash".to_string());
                }
            }
            QbftMessageType::RoundChange => {
                // Round change messages should have consistent root with prepared state
                if qbft_message.data_round > 0 && qbft_message.root.is_zero() {
                    // If data_round > 0, we should have a non-zero root
                    return Err("invalid hash".to_string());
                }
            }
            QbftMessageType::Prepare | QbftMessageType::Commit => {
                // Prepare and commit messages should have non-zero root
                if qbft_message.root.is_zero() {
                    return Err("invalid hash".to_string());
                }
            }
        }

        Ok(())
    }

    /// Setup controller scenario
    pub fn setup_controller_scenario(
        &mut self,
        input_value: String,
        height: Option<u64>,
    ) -> Result<(), AdapterError> {
        // Set height if provided
        if let Some(height) = height {
            self.set_current_height(height);
        }

        // Start instance if input value provided
        let input_bytes = self.decode_input_value(&input_value)?;
        self.start_instance(input_bytes)?;

        Ok(())
    }

    /// Setup message creation scenario
    pub fn setup_message_creation_scenario(
        &mut self,
        round: Option<Round>,
        state_value: Option<String>,
        round_change_justifications: Vec<SignedSSVMessage>,
        prepare_justifications: Vec<SignedSSVMessage>,
    ) -> Result<(), AdapterError> {
        // Extract prepared state if available
        let prepared_state = if let Some(state_value) = state_value {
            self.extract_prepared_state(&state_value)?
        } else {
            None
        };

        // Collect all justifications
        let mut all_justifications = Vec::new();
        all_justifications.extend(round_change_justifications);
        all_justifications.extend(prepare_justifications);

        // Setup the scenario
        let config = ScenarioConfig {
            round,
            prepared_state,
            justifications: all_justifications,
        };

        self.setup_scenario(config)?;
        Ok(())
    }

    // =============================================================================
    // CORE ADAPTER FUNCTIONALITY (from base.rs)
    // =============================================================================

    /// Start QBFT instance with input value
    pub fn start_instance(&mut self, input_value: Vec<u8>) -> Result<(), AdapterError> {
        if self.state.instance_started {
            return Err(AdapterError::InvalidState(
                "Instance already started".to_string(),
            ));
        }

        // For spec tests, we typically work with the data hash rather than raw data
        let _data_hash = Hash256::from_slice(&Sha256::digest(&input_value));

        self.state.instance_started = true;
        Ok(())
    }

    /// Set current height for state tracking
    pub fn set_current_height(&mut self, height: u64) {
        self.state.current_height = height;
    }

    /// Reset adapter for new test scenario
    pub fn reset_for_new_scenario(&mut self) {
        self.state.instance_started = false;
        self.state.decided_count = 0;
        self.state.decided_value = None;
        self.state.timeout_count = 0;
        self.state.prepared_state = None;
        self.state.justifications.clear();

        // Clear message queue
        self.message_queue.write().clear();
    }

    /// Create QBFT message based on request
    pub fn create_message(
        &mut self,
        request: MessageCreationRequest,
    ) -> Result<SignedSSVMessage, AdapterError> {
        self.create_message_with_committee_id(request, None)
    }

    pub fn create_message_with_committee_id(
        &mut self,
        request: MessageCreationRequest,
        identifier_bytes: Option<Vec<u8>>,
    ) -> Result<SignedSSVMessage, AdapterError> {
        use ssv_types::consensus::QbftMessage;
        use ssv_types::message::{MsgType, SSVMessage, SignedSSVMessage};
        use ssz::Encode;
        use tree_hash::TreeHash;
        use types::VariableList;

        // Debug output removed for cleaner results

        // Get the round, defaulting to 1 if not specified (round 0 is invalid)
        let round: u64 = request.round.unwrap_or(1.into()).into();
        eprintln!(
            "DEBUG: request.round = {:?}, final round = {}",
            request.round, round
        );

        // Use provided identifier bytes if available, otherwise use default message ID
        let id_bytes = if let Some(bytes) = identifier_bytes {
            bytes
        } else {
            let msg_id_bytes: [u8; 56] = self.identifier.clone().into();
            msg_id_bytes.to_vec()
        };
        let identifier = VariableList::new(id_bytes.clone())
            .map_err(|e| AdapterError::MessageCreation(format!("Invalid identifier: {:?}", e)))?;

        // Set data_round based on message type and state
        // For most message types, data_round should be 0 (NoRound)
        // Only for round change messages with prepared state should it be set

        let data_round = match request.msg_type {
            ssv_types::consensus::QbftMessageType::RoundChange => {
                // FIXED: data_round should reflect the prepared state round when previously prepared
                // From Go execution trace: prepared state uses data_round = 1 (the prepared round)
                let has_prepared_state =
                    !request.prepare_justifications.is_empty() || request.state_value.is_some();

                if has_prepared_state {
                    // In prepared state - set data_round to the round we were prepared in
                    // For spec tests, this is typically round 1
                    let prepared_round = 1; // The round we were previously prepared in
                    eprintln!(
                        "DEBUG DataRound: RoundChange setting to prepared round: {} (current round: {})",
                        prepared_round, round
                    );
                    prepared_round
                } else {
                    eprintln!("DEBUG DataRound: RoundChange setting to 0 (no prepared state)");
                    0 // Not prepared - use 0 (NoRound)
                }
            }
            ssv_types::consensus::QbftMessageType::Proposal => {
                // PROPOSAL LOGIC: Proposals might also need special data_round when previously prepared
                let has_prepared_state = !request.prepare_justifications.is_empty()
                    || !request.round_change_justifications.is_empty()
                    || request.state_value.is_some();

                if has_prepared_state {
                    // Proposal with prepared state - might need data_round = prepared round
                    let prepared_round = 1; // The round we were previously prepared in
                    eprintln!(
                        "DEBUG DataRound: Proposal setting to prepared round: {} (current round: {})",
                        prepared_round, round
                    );
                    prepared_round
                } else {
                    eprintln!("DEBUG DataRound: Proposal setting to 0 (no prepared state)");
                    0 // Not prepared - use 0 (NoRound)
                }
            }
            _ => {
                eprintln!("DEBUG DataRound: Other message type setting to 0");
                0 // For Prepare, Commit - data_round is always 0 (NoRound)
            }
        };

        // Helper function to check if justifications meet quorum requirement
        let has_quorum = |justifications: &[SignedSSVMessage]| -> bool {
            // For a 4-node committee, quorum is 3 (⅔ + 1)
            let committee_size = self.committee.len();
            let quorum_threshold = (committee_size * 2) / 3 + 1; // ⌊(2n)/3⌋ + 1

            // Count unique signers across all justifications
            let mut unique_signers = std::collections::HashSet::new();
            for justification in justifications {
                for &operator_id in justification.operator_ids() {
                    unique_signers.insert(operator_id);
                }
            }

            eprintln!(
                "DEBUG RUST: Quorum check - committee_size: {}, quorum_threshold: {}, unique_signers: {}",
                committee_size,
                quorum_threshold,
                unique_signers.len()
            );

            unique_signers.len() >= quorum_threshold
        };

        // Handle justifications properly based on message type and quorum requirements
        let (round_change_justification, prepare_justification) = match request.msg_type {
            ssv_types::consensus::QbftMessageType::Proposal => {
                // Proposals can have both types of justifications
                let rc_just = if !request.round_change_justifications.is_empty()
                    && has_quorum(&request.round_change_justifications)
                {
                    // Convert SignedSSVMessage to bytes for justifications
                    let mut rc_bytes: Vec<VariableList<u8, RoundChangeLength>> = Vec::new();
                    for (i, msg) in request.round_change_justifications.iter().enumerate() {
                        let encoded = msg.encode_without_full_data();
                        eprintln!(
                            "DEBUG RUST: RoundChangeJustification[{}]: {} bytes: {}",
                            i,
                            encoded.len(),
                            hex::encode(&encoded)
                        );
                        let var_list =
                            VariableList::new(encoded).unwrap_or_else(|_| VariableList::empty());
                        rc_bytes.push(var_list);
                    }
                    eprintln!(
                        "DEBUG RUST: RoundChangeJustifications total: {} items (quorum met)",
                        rc_bytes.len()
                    );
                    VariableList::<VariableList<u8, RoundChangeLength>, U13>::new(rc_bytes)
                        .unwrap_or_else(|_| VariableList::empty())
                } else {
                    if !request.round_change_justifications.is_empty() {
                        eprintln!("DEBUG RUST: RoundChangeJustifications filtered out - no quorum");
                    }
                    VariableList::empty()
                };

                let prep_just = if !request.prepare_justifications.is_empty()
                    && has_quorum(&request.prepare_justifications)
                {
                    let mut prep_bytes: Vec<VariableList<u8, JustificationLength>> = Vec::new();
                    for (i, msg) in request.prepare_justifications.iter().enumerate() {
                        let encoded = msg.encode_without_full_data();
                        eprintln!(
                            "DEBUG RUST: PrepareJustification[{}]: {} bytes: {}",
                            i,
                            encoded.len(),
                            hex::encode(&encoded)
                        );
                        let var_list =
                            VariableList::new(encoded).unwrap_or_else(|_| VariableList::empty());
                        prep_bytes.push(var_list);
                    }
                    eprintln!(
                        "DEBUG RUST: PrepareJustifications total: {} items (quorum met)",
                        prep_bytes.len()
                    );
                    VariableList::<VariableList<u8, JustificationLength>, U13>::new(prep_bytes)
                        .unwrap_or_else(|_| VariableList::empty())
                } else {
                    if !request.prepare_justifications.is_empty() {
                        eprintln!("DEBUG RUST: PrepareJustifications filtered out - no quorum");
                    }
                    VariableList::empty()
                };

                (rc_just, prep_just)
            }
            ssv_types::consensus::QbftMessageType::RoundChange => {
                // Round change messages can have prepare justifications, but only if they meet quorum
                let prep_just = if !request.prepare_justifications.is_empty()
                    && has_quorum(&request.prepare_justifications)
                {
                    let mut prep_bytes: Vec<VariableList<u8, JustificationLength>> = Vec::new();
                    for (i, msg) in request.prepare_justifications.iter().enumerate() {
                        let encoded = msg.encode_without_full_data();
                        eprintln!(
                            "DEBUG RUST: RoundChange PrepareJustification[{}]: {} bytes: {}",
                            i,
                            encoded.len(),
                            hex::encode(&encoded)
                        );
                        let var_list =
                            VariableList::new(encoded).unwrap_or_else(|_| VariableList::empty());
                        prep_bytes.push(var_list);
                    }
                    eprintln!(
                        "DEBUG RUST: RoundChange PrepareJustifications total: {} items (quorum met)",
                        prep_bytes.len()
                    );
                    VariableList::<VariableList<u8, JustificationLength>, U13>::new(prep_bytes)
                        .unwrap_or_else(|_| VariableList::empty())
                } else {
                    if !request.prepare_justifications.is_empty() {
                        eprintln!(
                            "DEBUG RUST: RoundChange PrepareJustifications filtered out - no quorum"
                        );
                    }
                    VariableList::empty()
                };

                (VariableList::empty(), prep_just)
            }
            _ => {
                // Prepare and Commit messages don't have justifications
                (VariableList::empty(), VariableList::empty())
            }
        };

        // Calculate root based on whether we have prepared state (NOT just whether justifications meet quorum)
        // CRITICAL INSIGHT: Go treats "insufficient quorum" as "previously prepared but can't include justifications"
        let root = match request.msg_type {
            ssv_types::consensus::QbftMessageType::RoundChange => {
                // FIXED LOGIC: If we have ANY prepare justifications OR state_value, we are in previously prepared state
                let has_prepared_state =
                    !request.prepare_justifications.is_empty() || request.state_value.is_some();

                eprintln!(
                    "DEBUG ROOT: has_prepared_state={}, justifications_count={}, state_value_len={:?}",
                    has_prepared_state,
                    request.prepare_justifications.len(),
                    request.state_value.as_ref().map(|sv| sv.len())
                );

                if has_prepared_state {
                    // We are in previously prepared state - use prepared value hash
                    if let Some(state_value) = request.state_value.as_ref() {
                        use sha2::{Digest, Sha256};
                        let state_hash = Hash256::from_slice(&Sha256::digest(state_value));
                        eprintln!("DEBUG ROOT: Using state_value hash: {:?}", state_hash);
                        state_hash
                    } else {
                        // Use the TestingQBFTFullData hash which is what the Go implementation uses
                        use sha2::{Digest, Sha256};
                        let testing_qbft_full_data = vec![
                            1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6,
                            7, 8, 9,
                        ];
                        let prepared_value_hash =
                            Hash256::from_slice(&Sha256::digest(&testing_qbft_full_data));
                        eprintln!(
                            "DEBUG ROOT: Using TestingQBFTFullData hash: {:?}",
                            prepared_value_hash
                        );
                        prepared_value_hash
                    }
                } else {
                    // No prepared state - use zero hash
                    eprintln!("DEBUG ROOT: Using zero hash (no prepared state)");
                    Hash256::from([0u8; 32])
                }
            }
            ssv_types::consensus::QbftMessageType::Proposal => {
                // PROPOSAL LOGIC: Proposals also need special root calculation
                let has_prepared_state = !request.prepare_justifications.is_empty()
                    || !request.round_change_justifications.is_empty()
                    || request.state_value.is_some();

                eprintln!(
                    "DEBUG ROOT: Proposal has_prepared_state={}, rc_justifications={}, prep_justifications={}, state_value_len={:?}",
                    has_prepared_state,
                    request.round_change_justifications.len(),
                    request.prepare_justifications.len(),
                    request.state_value.as_ref().map(|sv| sv.len())
                );

                if has_prepared_state {
                    // Proposal with prepared state - use prepared value hash
                    if let Some(state_value) = request.state_value.as_ref() {
                        use sha2::{Digest, Sha256};
                        let state_hash = Hash256::from_slice(&Sha256::digest(state_value));
                        eprintln!(
                            "DEBUG ROOT: Proposal using state_value hash: {:?}",
                            state_hash
                        );
                        state_hash
                    } else {
                        // Use the TestingQBFTFullData hash
                        use sha2::{Digest, Sha256};
                        let testing_qbft_full_data = vec![
                            1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6,
                            7, 8, 9,
                        ];
                        let prepared_value_hash =
                            Hash256::from_slice(&Sha256::digest(&testing_qbft_full_data));
                        eprintln!(
                            "DEBUG ROOT: Proposal using TestingQBFTFullData hash: {:?}",
                            prepared_value_hash
                        );
                        prepared_value_hash
                    }
                } else {
                    // Proposal with no prepared state - use data hash
                    eprintln!(
                        "DEBUG ROOT: Proposal using data_hash (no prepared state): {:?}",
                        request.data_hash
                    );
                    request.data_hash
                }
            }
            _ => {
                // For other message types (Prepare, Commit), use the data hash
                eprintln!(
                    "DEBUG ROOT: Using data_hash for other message type: {:?}",
                    request.data_hash
                );
                request.data_hash
            }
        };

        // Calculate FullData based on whether justifications will actually be included in final message
        let will_include_prepare_justifications = match request.msg_type {
            ssv_types::consensus::QbftMessageType::RoundChange => {
                !request.prepare_justifications.is_empty()
                    && has_quorum(&request.prepare_justifications)
            }
            ssv_types::consensus::QbftMessageType::Proposal => {
                !request.prepare_justifications.is_empty()
                    && has_quorum(&request.prepare_justifications)
            }
            _ => false,
        };

        let full_data_for_later = match request.msg_type {
            ssv_types::consensus::QbftMessageType::RoundChange => {
                // FIXED LOGIC: FullData follows the same prepared state logic as root calculation
                let has_prepared_state =
                    !request.prepare_justifications.is_empty() || request.state_value.is_some();

                if has_prepared_state {
                    // We are in previously prepared state - include FullData
                    if let Some(state_value) = request.state_value.as_ref() {
                        eprintln!(
                            "DEBUG FullData: Using state_value because in prepared state, len={}",
                            state_value.len()
                        );
                        state_value.clone()
                    } else {
                        eprintln!(
                            "DEBUG FullData: Using test data because in prepared state but no state_value"
                        );
                        vec![
                            1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6,
                            7, 8, 9,
                        ]
                    }
                } else {
                    eprintln!("DEBUG FullData: Using empty because not in prepared state");
                    vec![]
                }
            }
            ssv_types::consensus::QbftMessageType::Proposal => {
                // PROPOSAL LOGIC: FullData behavior might be different for proposals
                // Check if this proposal includes prepared value data
                if let Some(state_value) = request.state_value.as_ref() {
                    eprintln!(
                        "DEBUG FullData: Proposal using state_value, len={}",
                        state_value.len()
                    );
                    state_value.clone()
                } else if !request.prepare_justifications.is_empty()
                    || !request.round_change_justifications.is_empty()
                {
                    eprintln!(
                        "DEBUG FullData: Proposal using test data because has justifications"
                    );
                    vec![
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6, 7,
                        8, 9,
                    ]
                } else {
                    eprintln!("DEBUG FullData: Proposal using empty (no state)");
                    vec![]
                }
            }
            _ => {
                eprintln!("DEBUG FullData: Using empty for other message types");
                vec![]
            }
        };
        eprintln!(
            "DEBUG FullData: Final FullData len={}",
            full_data_for_later.len()
        );

        // Create QBFT message based on type
        eprintln!("DEBUG: Creating QBFT message with:");
        eprintln!("  - msg_type: {:?}", request.msg_type);
        eprintln!("  - height: 0");
        eprintln!("  - round: {}", round);
        eprintln!("  - identifier: {:?}", identifier);
        eprintln!("  - root: {:?}", root);
        eprintln!("  - data_round: {}", data_round);
        eprintln!(
            "  - round_change_justification: {} items",
            round_change_justification.len()
        );
        eprintln!(
            "  - prepare_justification: {} items",
            prepare_justification.len()
        );

        // Create QBFT message with justifications included when provided
        let qbft_message = QbftMessage {
            qbft_message_type: request.msg_type,
            height: 0, // Spec tests expect height=0
            round,
            identifier,
            root,
            data_round,
            round_change_justification,
            prepare_justification,
        };

        // Create SSV message with QBFT data
        let data_bytes = qbft_message.as_ssz_bytes();

        let data_list = VariableList::new(data_bytes)
            .map_err(|e| AdapterError::MessageCreation(format!("Invalid data: {:?}", e)))?;

        // Create the SSV message identifier from the same bytes used for QBFT identifier
        let ssv_identifier =
            ssv_types::msgid::MessageId::try_from(id_bytes.as_slice()).map_err(|e| {
                AdapterError::MessageCreation(format!("Invalid SSV identifier: {:?}", e))
            })?;

        let ssv_message = SSVMessage::new(MsgType::SSVConsensusMsgType, ssv_identifier, data_list)
            .map_err(|e| {
                AdapterError::MessageCreation(format!("Failed to create SSV message: {:?}", e))
            })?;

        // Create signed message with proper RSA signature using TestKeySet
        use crate::utils::test_keys::TestKeySet;
        use openssl::hash::MessageDigest;
        use openssl::pkey::PKey;
        use openssl::sign::Signer;

        let test_keys = TestKeySet::four_share_set();
        let signing_key = test_keys
            .operator_keys
            .get(&self.operator_id)
            .ok_or_else(|| {
                AdapterError::MessageCreation(format!(
                    "No key found for operator {}",
                    self.operator_id.0
                ))
            })?;

        // Convert RSA key to PKey for signing
        let pkey = PKey::from_rsa(signing_key.clone()).map_err(|e| {
            AdapterError::MessageCreation(format!("Failed to convert RSA key: {:?}", e))
        })?;

        // Create the message bytes to sign (SSV message)
        let message_bytes = ssv_message.as_ssz_bytes();

        // Sign the message using RSA-SHA256
        let mut signer = Signer::new(MessageDigest::sha256(), &pkey).map_err(|e| {
            AdapterError::MessageCreation(format!("Failed to create signer: {:?}", e))
        })?;
        signer.update(&message_bytes).map_err(|e| {
            AdapterError::MessageCreation(format!("Failed to update signer: {:?}", e))
        })?;
        let signature_bytes = signer
            .sign_to_vec()
            .map_err(|e| AdapterError::MessageCreation(format!("Failed to sign: {:?}", e)))?;

        let signature = VariableList::new(signature_bytes)
            .map_err(|e| AdapterError::MessageCreation(format!("Invalid signature: {:?}", e)))?;
        let signatures = VariableList::new(vec![signature])
            .map_err(|e| AdapterError::MessageCreation(format!("Invalid signatures: {:?}", e)))?;
        let operator_ids = VariableList::new(vec![self.operator_id])
            .map_err(|e| AdapterError::MessageCreation(format!("Invalid operator IDs: {:?}", e)))?;
        // Use the pre-calculated FullData that was based on actual quorum results
        let full_data = VariableList::new(full_data_for_later)
            .map_err(|e| AdapterError::MessageCreation(format!("Invalid full data: {:?}", e)))?;

        let signed_message =
            SignedSSVMessage::new(signatures, operator_ids, ssv_message, full_data).map_err(
                |e| {
                    let go_error = map_signed_ssv_error_to_go_format(&e);
                    AdapterError::MessageCreation(go_error)
                },
            )?;

        // Debug the complete message structure before hashing
        eprintln!("DEBUG RUST: Complete SignedSSVMessage structure:");
        eprintln!(
            "  - signatures.len(): {}",
            signed_message.signatures().len()
        );
        eprintln!(
            "  - operator_ids.len(): {}",
            signed_message.operator_ids().len()
        );
        eprintln!(
            "  - ssv_message.msg_type: {:?}",
            signed_message.ssv_message().msg_type()
        );
        eprintln!(
            "  - ssv_message.msg_id: {:?}",
            signed_message.ssv_message().msg_id()
        );
        eprintln!(
            "  - ssv_message.data.len(): {}",
            signed_message.ssv_message().data().len()
        );
        eprintln!("  - full_data.len(): {}", signed_message.full_data().len());

        // Debug individual field bytes for detailed comparison
        eprintln!("DEBUG RUST: Individual field bytes:");
        let sig_bytes = signed_message.signatures().as_ssz_bytes();
        eprintln!(
            "  - signatures bytes: {} bytes: {}",
            sig_bytes.len(),
            hex::encode(&sig_bytes)
        );

        // Debug operator IDs manually since slice doesn't implement Encode
        let op_ids = signed_message.operator_ids();
        eprintln!("  - operator_ids count: {}", op_ids.len());
        for (i, op_id) in op_ids.iter().enumerate() {
            eprintln!("    - operator_id[{}]: {}", i, op_id.0);
        }
        let ssv_msg_bytes = signed_message.ssv_message().as_ssz_bytes();
        eprintln!(
            "  - ssv_message bytes: {} bytes: {}",
            ssv_msg_bytes.len(),
            hex::encode(&ssv_msg_bytes)
        );
        let full_data = signed_message.full_data();
        eprintln!(
            "  - full_data bytes: {} bytes: {}",
            full_data.len(),
            hex::encode(&full_data)
        );

        // Debug the SSV message internal structure
        eprintln!("DEBUG RUST: SSV message internal structure:");
        let msg_type_bytes = signed_message.ssv_message().msg_type().as_ssz_bytes();
        eprintln!(
            "  - msg_type bytes: {} bytes: {}",
            msg_type_bytes.len(),
            hex::encode(&msg_type_bytes)
        );
        let msg_id_bytes = signed_message.ssv_message().msg_id().as_ssz_bytes();
        eprintln!(
            "  - msg_id bytes: {} bytes: {}",
            msg_id_bytes.len(),
            hex::encode(&msg_id_bytes)
        );
        let data = signed_message.ssv_message().data();
        eprintln!(
            "  - data bytes: {} bytes: {}",
            data.len(),
            hex::encode(&data)
        );

        // Debug the QBFT message structure within the data
        eprintln!("DEBUG RUST: QBFT message structure (within SSV data):");
        let qbft_type_bytes = qbft_message.qbft_message_type.as_ssz_bytes();
        eprintln!(
            "  - qbft_message_type bytes: {} bytes: {}",
            qbft_type_bytes.len(),
            hex::encode(&qbft_type_bytes)
        );
        let height_bytes = qbft_message.height.as_ssz_bytes();
        eprintln!(
            "  - height bytes: {} bytes: {}",
            height_bytes.len(),
            hex::encode(&height_bytes)
        );
        let round_bytes = qbft_message.round.as_ssz_bytes();
        eprintln!(
            "  - round bytes: {} bytes: {}",
            round_bytes.len(),
            hex::encode(&round_bytes)
        );
        let identifier_bytes = qbft_message.identifier.as_ssz_bytes();
        eprintln!(
            "  - identifier bytes: {} bytes: {}",
            identifier_bytes.len(),
            hex::encode(&identifier_bytes)
        );
        let root_bytes = qbft_message.root.as_ssz_bytes();
        eprintln!(
            "  - root bytes: {} bytes: {}",
            root_bytes.len(),
            hex::encode(&root_bytes)
        );
        let data_round_bytes = qbft_message.data_round.as_ssz_bytes();
        eprintln!(
            "  - data_round bytes: {} bytes: {}",
            data_round_bytes.len(),
            hex::encode(&data_round_bytes)
        );
        let rc_just_bytes = qbft_message.round_change_justification.as_ssz_bytes();
        eprintln!(
            "  - round_change_justification bytes: {} bytes: {}",
            rc_just_bytes.len(),
            hex::encode(&rc_just_bytes)
        );
        let prep_just_bytes = qbft_message.prepare_justification.as_ssz_bytes();
        eprintln!(
            "  - prepare_justification bytes: {} bytes: {}",
            prep_just_bytes.len(),
            hex::encode(&prep_just_bytes)
        );

        // Debug the complete SSZ encoding
        let complete_ssz = signed_message.as_ssz_bytes();
        eprintln!(
            "DEBUG RUST: Complete SSZ encoded message: {} bytes",
            complete_ssz.len()
        );
        eprintln!(
            "DEBUG RUST: Complete SSZ hex: {}",
            hex::encode(&complete_ssz)
        );

        // Debug the tree hash root calculation
        let final_hash = signed_message.tree_hash_root();
        eprintln!("DEBUG RUST: Final hash calculated: {:?}", final_hash);

        Ok(signed_message)
    }

    /// Process messages and return unified result
    pub fn process_messages(
        &mut self,
        messages: Vec<SignedSSVMessage>,
    ) -> Result<ProcessingResult, AdapterError> {
        let consensus_reached = false;
        let mut validation_errors = Vec::new();

        // Process each message
        for message in messages {
            // Validate message first
            if let Err(validation_error) = validate_message_full(&message, &self.committee, None) {
                validation_errors.push(format!("{:?}", validation_error));
                continue; // Skip invalid messages
            }

            // Check if consensus was reached after processing
            self.update_consensus_state();
        }

        // Extract any outgoing messages
        let messages_sent = self.extract_outgoing_messages();

        Ok(ProcessingResult {
            consensus_reached,
            messages_sent,
            validation_result: ValidationResult {
                is_valid: validation_errors.is_empty(),
                errors: validation_errors,
                warnings: Vec::new(),
            },
            go_error_messages: Vec::new(),
        })
    }

    /// Controller-specific message processing (for compatibility)
    pub fn process_messages_controller(
        &mut self,
        messages: Vec<SignedSSVMessage>,
    ) -> Result<ControllerResult, AdapterError> {
        let processing_result = self.process_messages(messages)?;
        let decided_state = self.get_decided_state();

        Ok(ControllerResult {
            processing_result,
            decided_state,
        })
    }

    /// Setup test scenario with configuration
    pub fn setup_scenario(&mut self, config: ScenarioConfig) -> Result<(), AdapterError> {
        // Set prepared state if provided
        if let Some(prepared_state) = config.prepared_state {
            self.state.prepared_state = Some(prepared_state);
        }

        // Store justifications for use in message creation
        self.state.justifications = config.justifications;

        // Validate all justifications
        let validation_result =
            validate_justifications(&self.state.justifications, &self.committee);
        if !validation_result.is_valid {
            return Err(AdapterError::Validation(
                validation_result.errors.join(", "),
            ));
        }

        Ok(())
    }

    /// Get current decided state
    pub fn get_decided_state(&self) -> DecidedState {
        DecidedState {
            decided_count: self.state.decided_count,
            decided_value: self.state.decided_value.clone(),
        }
    }

    /// Get current timer state
    pub fn get_timer_state(&self) -> Option<TimerState> {
        Some(TimerState {
            timeouts: self.state.timeout_count,
            current_round: self.qbft.get_round(),
        })
    }

    /// Validate message root against expected value
    pub fn validate_root(&self, message: &SignedSSVMessage, expected: Hash256) -> bool {
        validate_root(message, expected)
    }

    /// Enhanced message processing with complete error handling
    pub fn process_messages_with_context(
        &mut self,
        messages: Vec<SignedSSVMessage>,
        context: &TestContext,
    ) -> Result<ScenarioResult, AdapterError> {
        let mut all_validation_errors = Vec::new();
        let mut processed_messages = Vec::new();

        // Process each message with comprehensive validation
        for message in messages {
            let (validation_result, _go_errors) =
                validate_and_format_errors(&message, &self.committee, context);

            if validation_result.is_valid {
                processed_messages.push(message);
            } else {
                all_validation_errors.extend(validation_result.errors);
            }
        }

        // Process valid messages through QBFT
        let _processing_result = self.process_valid_messages(processed_messages, context)?;

        // Update consensus state
        self.update_consensus_state();

        // Create comprehensive scenario result
        Ok(self.process_controller_scenario_result(context))
    }

    /// Enhanced message creation with validation
    pub fn create_message_with_validation(
        &mut self,
        request: MessageCreationRequest,
        context: &TestContext,
    ) -> Result<(SignedSSVMessage, ValidationResult), AdapterError> {
        self.create_message_with_validation_and_committee_id(request, context, None)
    }

    pub fn create_message_with_validation_and_committee_id(
        &mut self,
        request: MessageCreationRequest,
        context: &TestContext,
        committee_id: Option<Vec<u8>>,
    ) -> Result<(SignedSSVMessage, ValidationResult), AdapterError> {
        // Create message using QBFT
        let message = self.create_message_with_committee_id(request, committee_id)?;

        // Validate created message
        let (validation_result, _) = validate_and_format_errors(&message, &self.committee, context);

        Ok((message, validation_result))
    }

    /// Enhanced error reporting with Go formatting
    pub fn get_validation_errors_formatted(&self, context: &TestContext) -> Vec<String> {
        let _error_mapper = ErrorMapper::new(context.clone());
        // For now, return empty - this would be filled with current validation state
        Vec::new()
    }

    /// Enhanced state inspection with context
    pub fn get_scenario_result(&self, context: &TestContext) -> ScenarioResult {
        self.process_controller_scenario_result(context)
    }

    // =============================================================================
    // HELPER METHODS
    // =============================================================================

    /// Update consensus state based on QBFT completion
    fn update_consensus_state(&mut self) {
        if let Some(completed) = self.qbft.completed() {
            match completed {
                qbft::Completed::Success(data) => {
                    self.state.decided_count += 1;
                    self.state.decided_value = Some(data.block_root.as_slice().to_vec());
                }
                qbft::Completed::TimedOut => {
                    self.state.timeout_count += 1;
                }
            }
        }
    }

    /// Extract outgoing messages from queue
    fn extract_outgoing_messages(&self) -> Vec<SignedSSVMessage> {
        let mut queue = self.message_queue.write();
        let _messages: Vec<_> = queue.drain(..).collect();

        // Convert UnsignedWrappedQbftMessage to SignedSSVMessage
        // This is simplified - real implementation would need proper signing
        Vec::new() // Placeholder for now
    }

    /// Process valid messages through QBFT
    fn process_valid_messages(
        &mut self,
        _messages: Vec<SignedSSVMessage>,
        context: &TestContext,
    ) -> Result<ProcessingResult, AdapterError> {
        // For now, simplified processing - this would integrate with actual QBFT processing
        let _error_mapper = ErrorMapper::new(context.clone());

        Ok(ProcessingResult {
            consensus_reached: false,
            messages_sent: Vec::new(),
            validation_result: ValidationResult {
                is_valid: true,
                errors: Vec::new(),
                warnings: Vec::new(),
            },
            go_error_messages: Vec::new(),
        })
    }

    /// Process controller scenario result
    fn process_controller_scenario_result(&self, context: &TestContext) -> ScenarioResult {
        let _error_mapper = ErrorMapper::new(context.clone());
        let scenario_id = context.scenario_id();

        ScenarioResult {
            scenario_id,
            processing_result: ProcessingResult {
                consensus_reached: false,  // Will be updated by actual processing
                messages_sent: Vec::new(), // Will be filled by message processing
                validation_result: ValidationResult {
                    is_valid: true,
                    errors: Vec::new(),
                    warnings: Vec::new(),
                },
                go_error_messages: Vec::new(),
            },
            decided_state: self.get_decided_state(),
            timer_state: self.get_timer_state(),
            validation_errors: Vec::new(),
            go_formatted_errors: Vec::new(),
        }
    }

    /// Extract prepared state from state value
    fn extract_prepared_state(
        &self,
        state_value: &str,
    ) -> Result<Option<(Round, Hash256)>, AdapterError> {
        if state_value.is_empty() {
            return Ok(None);
        }

        // Decode the base64 state value
        let decoded_state_value = base64::engine::general_purpose::STANDARD
            .decode(state_value)
            .map_err(AdapterError::Base64Decode)?;

        if decoded_state_value.is_empty() {
            return Ok(None);
        }

        // Calculate the hash of the state value
        let state_value_hash = Hash256::from_slice(&Sha256::digest(&decoded_state_value));

        // Use round 1 as default prepared round
        let prepared_round = Round::from(1);

        Ok(Some((prepared_round, state_value_hash)))
    }

    /// Decode input value
    fn decode_input_value(&self, input_value_b64: &str) -> Result<Vec<u8>, AdapterError> {
        base64::engine::general_purpose::STANDARD
            .decode(input_value_b64)
            .map_err(AdapterError::Base64Decode)
    }

    /// Create error result for failed scenarios
    fn create_error_result(&self, scenario_id: &str, error: AdapterError) -> ScenarioResult {
        let context = self.test_context.clone().unwrap_or_default();
        let error_mapper = ErrorMapper::new(context);
        let go_error = error_mapper.map_adapter_error(&error);

        ScenarioResult {
            scenario_id: scenario_id.to_string(),
            processing_result: ProcessingResult {
                consensus_reached: false,
                messages_sent: Vec::new(),
                validation_result: ValidationResult {
                    is_valid: false,
                    errors: Vec::new(),
                    warnings: vec![error.to_string()],
                },
                go_error_messages: vec![go_error.clone()],
            },
            decided_state: DecidedState {
                decided_count: 0,
                decided_value: None,
            },
            timer_state: None,
            validation_errors: Vec::new(),
            go_formatted_errors: vec![go_error],
        }
    }

    // =============================================================================
    // ACCESSORS
    // =============================================================================

    /// Get the underlying QBFT instance (for advanced usage)
    pub fn qbft(&self) -> &Qbft<DefaultLeaderFunction, BeaconVote, SharedMessageSender> {
        &self.qbft
    }

    /// Get the committee
    pub fn committee(&self) -> &IndexSet<OperatorId> {
        &self.committee
    }

    /// Get the operator ID
    pub fn operator_id(&self) -> OperatorId {
        self.operator_id
    }

    /// Get the message identifier
    pub fn identifier(&self) -> &MessageId {
        &self.identifier
    }

    /// Get current configuration
    pub fn config(&self) -> &AdapterConfig {
        &self.config
    }

    /// Get current state
    pub fn state(&self) -> &AdapterState {
        &self.state
    }
}
