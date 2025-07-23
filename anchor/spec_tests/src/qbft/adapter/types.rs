use openssl::rsa::Rsa;
use serde::Deserialize;
use ssv_types::{OperatorId, Round, message::SignedSSVMessage};
use std::collections::HashMap;
use types::Hash256;
use crate::utils::async_test_utils::{CommitteeInstanceId, ControllerStateData};

// =================== Message Processing Test Types ===================

/// Main test structure for message processing tests
#[derive(Debug, Clone, Deserialize)]
pub struct MsgProcessingTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Pre")]
    pub pre: MsgProcessingPre,
    #[serde(rename = "PostRoot")]
    pub post_root: Option<String>,
    #[serde(rename = "PostState")]
    pub post_state: Option<QbftInstanceState>,
    #[serde(rename = "OutputMessages")]
    pub output_messages: Vec<SignedSSVMessage>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: Option<String>,
}

/// Pre-test state for message processing
#[derive(Debug, Clone, Deserialize)]
pub struct MsgProcessingPre {
    #[serde(rename = "State")]
    pub state: QbftInstanceState,
    #[serde(rename = "InputMessages")]
    pub input_messages: Vec<MessageContainer>,
}

/// Complete QBFT instance state representation
#[derive(Debug, Clone, Deserialize)]
pub struct QbftInstanceState {
    #[serde(rename = "Height")]
    pub height: u64,
    #[serde(rename = "Round")]
    pub round: u64,
    #[serde(rename = "Stage")]
    pub stage: u8,
    #[serde(rename = "LastPreparedRound")]
    pub last_prepared_round: Option<u64>,
    #[serde(rename = "LastPreparedValue")]
    pub last_prepared_value: Option<Vec<u8>>,
    #[serde(rename = "ProposalAcceptedForCurrentRound")]
    pub proposal_accepted_for_current_round: Option<Vec<u8>>,
    #[serde(rename = "Decided")]
    pub decided: bool,
    #[serde(rename = "DecidedValue")]
    pub decided_value: Option<Vec<u8>>,
    #[serde(rename = "ProposeContainer")]
    pub propose_container: Option<MessageContainer>,
    #[serde(rename = "PrepareContainer")]
    pub prepare_container: Option<MessageContainer>,
    #[serde(rename = "CommitContainer")]
    pub commit_container: Option<MessageContainer>,
    #[serde(rename = "RoundChangeContainer")]
    pub round_change_container: Option<MessageContainer>,
}

/// Message container for different message types
#[derive(Debug, Clone, Deserialize)]
pub struct MessageContainer {
    #[serde(rename = "Msgs")]
    pub msgs: HashMap<String, SignedSSVMessage>,
}

/// Timer state information - enhanced from existing TimerState
#[derive(Debug, Clone, Deserialize)]
pub struct TimerState {
    #[serde(rename = "Timeouts")]
    pub timeouts: u64,
    #[serde(rename = "Round")]
    pub current_round: u64,
    #[serde(rename = "TimeoutF")]
    pub timeout_f: Option<u64>,
}

/// Processed message result for message sequence processing
#[derive(Debug, Clone)]
pub struct ProcessedMessage {
    /// The original message
    pub message: SignedSSVMessage,
    /// Whether the message was processed successfully
    pub processed: bool,
    /// The height at which this message was processed
    pub result_height: Option<u64>,
    /// Any error that occurred during processing
    pub error: Option<String>,
}

// =================== End Message Processing Test Types ===================

impl QbftInstanceState {
    /// Convert u64 round to ssv_types::Round
    pub fn round(&self) -> Round {
        Round::from(self.round)
    }

    /// Convert optional u64 to optional ssv_types::Round
    pub fn last_prepared_round(&self) -> Option<Round> {
        self.last_prepared_round.map(Round::from)
    }
}

impl TimerState {
    /// Convert u64 round to ssv_types::Round
    pub fn current_round(&self) -> Round {
        Round::from(self.current_round)
    }
}

/// Unified decided state for all QBFT test scenarios
#[derive(Debug, Clone)]
pub struct DecidedState {
    pub decided_count: u64,
    pub decided_value: Option<Vec<u8>>,
}


/// Validation result with comprehensive error information
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>, // Store error strings instead of ValidationFailure
    pub warnings: Vec<String>,
}

/// Unified processing result for message operations
#[derive(Debug, Clone)]
pub struct ProcessingResult {
    pub consensus_reached: bool,
    pub messages_sent: Vec<SignedSSVMessage>,
    pub validation_result: ValidationResult,
    pub go_error_messages: Vec<String>, // Pre-mapped for test assertions
}

/// Internal adapter state tracking
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

/// Request structure for message creation
#[derive(Debug, Clone)]
pub struct MessageCreationRequest {
    pub msg_type: ssv_types::consensus::QbftMessageType,
    pub data_hash: Hash256,
    pub round: Option<Round>,
    pub state_value: Option<Vec<u8>>,
    pub round_change_justifications: Vec<SignedSSVMessage>,
    pub prepare_justifications: Vec<SignedSSVMessage>,
}

/// Configuration for test scenario setup
#[derive(Debug, Clone)]
pub struct ScenarioConfig {
    pub round: Option<Round>,
    pub prepared_state: Option<(Round, Hash256)>,
    pub justifications: Vec<SignedSSVMessage>,
}

/// Configuration for adapter creation
#[derive(Debug, Clone)]
pub struct AdapterConfig {
    pub instance_height: u64,
    pub current_height: u64,
    pub committee_size: usize,
    pub quorum_threshold: usize,
    pub max_rounds: u64,
}

/// Minimal test keys structure for adapter testing
#[derive(Debug, Clone)]
pub struct TestKeys {
    pub operator_keys: HashMap<OperatorId, Rsa<openssl::pkey::Private>>,
    pub committee_size: usize,
}

impl TestKeys {
    /// Create a minimal test key set for 4-operator committee
    pub fn four_share_set() -> Self {
        let mut operator_keys = HashMap::new();

        // Generate minimal RSA keys for 4 operators
        for operator_id in 1..=4 {
            // Generate a 1024-bit RSA key for testing (smaller for performance)
            let rsa_key = Rsa::generate(1024).expect("Failed to generate RSA key for testing");
            operator_keys.insert(OperatorId::from(operator_id), rsa_key);
        }

        Self {
            operator_keys,
            committee_size: 4,
        }
    }

    /// Get signing key for specific operator
    pub fn get_key(&self, operator_id: OperatorId) -> Option<&Rsa<openssl::pkey::Private>> {
        self.operator_keys.get(&operator_id)
    }
}

/// Simplified error type for adapter operations
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("Message creation failed: {0}")]
    MessageCreation(String),
    #[error("Validation failed: {0}")]
    Validation(String),
    #[error("Key loading failed: {0}")]
    KeyLoading(String),
    #[error("Signing failed: {0}")]
    Signing(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("OpenSSL error: {0}")]
    OpenSsl(#[from] openssl::error::ErrorStack),
    #[error("Invalid state: {0}")]
    InvalidState(String),
    #[error("Base64 decode error: {0}")]
    Base64Decode(#[from] base64::DecodeError),
}

// Legacy spec test types (kept for JSON deserialization)
#[derive(Debug, Clone, Deserialize)]
pub struct SpecTestCommitteeMember {
    #[serde(rename = "OperatorID")]
    pub operator_id: OperatorId,
    #[serde(rename = "CommitteeID")]
    pub committee_id: Vec<u8>,
    #[serde(rename = "SSVOperatorPubKey")]
    pub ssv_operator_pub_key: String,
    #[serde(rename = "FaultyNodes")]
    pub faulty_nodes: u64,
    #[serde(rename = "Committee")]
    pub committee: Vec<SpecTestOperator>,
    #[serde(rename = "DomainType")]
    pub domain_type: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpecTestOperator {
    #[serde(rename = "OperatorID")]
    pub operator_id: u64,
    #[serde(rename = "SSVOperatorPubKey")]
    pub ssv_operator_pub_key: String,
}

impl Default for AdapterState {
    fn default() -> Self {
        Self {
            current_height: 0,
            instance_height: 0,
            instance_started: false,
            decided_count: 0,
            decided_value: None,
            timeout_count: 0,
            prepared_state: None,
            justifications: Vec::new(),
        }
    }
}

/// Test context for error mapping and scenario management
#[derive(Debug, Clone)]
pub struct TestContext {
    pub test_name: String,
    pub test_type: TestType,
    pub expected_errors: Vec<String>,
    pub error_mapping_context: HashMap<String, String>,
    // Additional fields for timeout testing
    pub instance_height: Option<u64>,
    pub current_round: Option<Round>,
    pub last_prepared_round: Option<u64>,
    pub last_prepared_value: Option<String>,
    pub decided: Option<bool>,
    pub decided_value: Option<String>,
}

/// Test type enumeration for context-aware processing
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum TestType {
    Controller,
    MessageCreation,
    MessageProcessing,
    QbftMessage,
    RoundRobin,
    Timeout,
}

/// Comprehensive result for test scenarios
#[derive(Debug, Clone)]
pub struct ScenarioResult {
    pub scenario_id: String,
    pub processing_result: ProcessingResult,
    pub decided_state: DecidedState,
    pub timer_state: Option<TimerState>,
    pub controller_root: Option<String>,
    pub validation_errors: Vec<String>, // Store error strings instead of ValidationFailure
    pub go_formatted_errors: Vec<String>,
}

impl Default for AdapterConfig {
    fn default() -> Self {
        Self {
            instance_height: 0,
            current_height: 0,
            committee_size: 4,
            quorum_threshold: 3,
            max_rounds: 100,
        }
    }
}

impl Default for TestContext {
    fn default() -> Self {
        Self {
            test_name: "default".to_string(),
            test_type: TestType::Controller,
            expected_errors: Vec::new(),
            error_mapping_context: HashMap::new(),
            instance_height: None,
            current_round: None,
            last_prepared_round: None,
            last_prepared_value: None,
            decided: None,
            decided_value: None,
        }
    }
}

impl TestContext {
    pub fn new(test_name: String, test_type: TestType) -> Self {
        Self {
            test_name,
            test_type,
            expected_errors: Vec::new(),
            error_mapping_context: HashMap::new(),
            instance_height: None,
            current_round: None,
            last_prepared_round: None,
            last_prepared_value: None,
            decided: None,
            decided_value: None,
        }
    }

    pub fn with_expected_errors(mut self, errors: Vec<String>) -> Self {
        self.expected_errors = errors;
        self
    }

    pub fn scenario_id(&self) -> String {
        format!("{}_{:?}", self.test_name, self.test_type)
    }

    pub fn default_controller() -> Self {
        Self {
            test_name: "controller_test".to_string(),
            test_type: TestType::Controller,
            expected_errors: Vec::new(),
            error_mapping_context: HashMap::new(),
            instance_height: None,
            current_round: None,
            last_prepared_round: None,
            last_prepared_value: None,
            decided: None,
            decided_value: None,
        }
    }

    pub fn default_message_creation() -> Self {
        Self {
            test_name: "message_creation_test".to_string(),
            test_type: TestType::MessageCreation,
            expected_errors: Vec::new(),
            error_mapping_context: HashMap::new(),
            instance_height: None,
            current_round: None,
            last_prepared_round: None,
            last_prepared_value: None,
            decided: None,
            decided_value: None,
        }
    }

    pub fn default_validation() -> Self {
        Self {
            test_name: "validation_test".to_string(),
            test_type: TestType::QbftMessage,
            expected_errors: Vec::new(),
            error_mapping_context: HashMap::new(),
            instance_height: None,
            current_round: None,
            last_prepared_round: None,
            last_prepared_value: None,
            decided: None,
            decided_value: None,
        }
    }
}

impl TestType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TestType::Controller => "controller",
            TestType::MessageCreation => "message_creation",
            TestType::MessageProcessing => "message_processing",
            TestType::QbftMessage => "qbft_message",
            TestType::RoundRobin => "round_robin",
            TestType::Timeout => "timeout",
        }
    }
}

/// Async result structure for QBFT manager test scenarios
#[derive(Debug, Clone)]
pub struct AsyncScenarioResult {
    pub scenario_id: String,
    pub decisions: Vec<AsyncDecisionResult>,
    pub controller_state: Option<ControllerStateData>,
    pub processing_errors: Vec<String>,
    pub decided_state: DecidedState,
    pub timer_state: Option<TimerState>,
    pub controller_root: Option<String>,
    pub validation_errors: Vec<String>,
    pub go_formatted_errors: Vec<String>,
}

/// Async decision result for individual QBFT instances
#[derive(Debug, Clone)]
pub struct AsyncDecisionResult {
    pub instance_id: CommitteeInstanceId,
    pub decided_value: Option<Vec<u8>>,
    pub messages_processed: usize,
}

// Convert to existing ScenarioResult for compatibility
impl From<AsyncScenarioResult> for ScenarioResult {
    fn from(async_result: AsyncScenarioResult) -> Self {
        ScenarioResult {
            scenario_id: async_result.scenario_id,
            processing_result: ProcessingResult {
                consensus_reached: async_result.decided_state.decided_count > 0,
                messages_sent: Vec::new(),
                validation_result: ValidationResult {
                    is_valid: async_result.processing_errors.is_empty(),
                    errors: async_result.processing_errors.clone(),
                    warnings: Vec::new(),
                },
                go_error_messages: async_result.go_formatted_errors.clone(),
            },
            decided_state: async_result.decided_state,
            timer_state: async_result.timer_state,
            controller_root: async_result.controller_root,
            validation_errors: async_result.validation_errors,
            go_formatted_errors: async_result.go_formatted_errors,
        }
    }
}
