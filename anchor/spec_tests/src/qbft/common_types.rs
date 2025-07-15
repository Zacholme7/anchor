//! Shared QBFT type definitions
//!
//! This module provides common type definitions used across QBFT spec tests.
//! It centralizes duplicate type definitions and provides a single source of truth
//! for committee members, operators, test configurations, and test scenarios.

use qbft::TestConfig;
use serde::{Deserialize, Serialize};
use ssv_types::{IndexSet, OperatorId, Round, message::SignedSSVMessage};
use types::Hash256;

use crate::utils::deserializers::type_parse::deserialize_base64_to_bytes;

/// Committee member structure representing a participant in the QBFT committee
///
/// This structure matches the JSON format from the spec tests and provides
/// all necessary information about a committee member including their operator ID,
/// committee information, public key, and fault tolerance configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitteeMember {
    /// The unique identifier for the operator
    #[serde(rename = "OperatorID")]
    pub operator_id: OperatorId,
    
    /// The committee identifier (typically a hash or unique identifier)
    #[serde(rename = "CommitteeID")]
    pub committee_id: Vec<u8>,
    
    /// The SSV operator public key (base64 encoded in JSON)
    #[serde(rename = "SSVOperatorPubKey")]
    #[serde(deserialize_with = "deserialize_base64_to_bytes")]
    pub ssv_operator_pub_key: Vec<u8>,
    
    /// Number of faulty nodes this committee can tolerate
    #[serde(rename = "FaultyNodes")]
    pub faulty_nodes: u64,
    
    /// The complete committee composition
    #[serde(rename = "Committee")]
    pub committee: Vec<Operator>,
    
    /// Domain type for the committee (4 bytes)
    #[serde(rename = "DomainType")]
    pub domain_type: Vec<u8>,
}

impl CommitteeMember {
    /// Create a new committee member with the given parameters
    pub fn new(
        operator_id: OperatorId,
        committee_id: Vec<u8>,
        ssv_operator_pub_key: Vec<u8>,
        faulty_nodes: u64,
        committee: Vec<Operator>,
        domain_type: Vec<u8>,
    ) -> Self {
        Self {
            operator_id,
            committee_id,
            ssv_operator_pub_key,
            faulty_nodes,
            committee,
            domain_type,
        }
    }

    /// Get the committee as an IndexSet of OperatorIds
    pub fn get_committee_set(&self) -> IndexSet<OperatorId> {
        self.committee.iter().map(|op| op.operator_id).collect()
    }

    /// Get the committee size
    pub fn committee_size(&self) -> usize {
        self.committee.len()
    }

    /// Calculate the quorum threshold for this committee
    pub fn quorum_threshold(&self) -> usize {
        (self.committee.len() * 2 / 3) + 1
    }

    /// Find an operator by ID in the committee
    pub fn find_operator(&self, operator_id: OperatorId) -> Option<&Operator> {
        self.committee.iter().find(|op| op.operator_id == operator_id)
    }

    /// Check if an operator is part of the committee
    pub fn contains_operator(&self, operator_id: OperatorId) -> bool {
        self.committee.iter().any(|op| op.operator_id == operator_id)
    }

    /// Get the domain type as a fixed-size array
    pub fn domain_type_array(&self) -> Option<[u8; 4]> {
        if self.domain_type.len() == 4 {
            let mut array = [0u8; 4];
            array.copy_from_slice(&self.domain_type);
            Some(array)
        } else {
            None
        }
    }
}

/// Operator structure representing a single operator in the committee
///
/// This structure contains the basic information needed to identify and
/// interact with an operator in the QBFT protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operator {
    /// The unique identifier for the operator
    #[serde(rename = "OperatorID")]
    pub operator_id: OperatorId,
    
    /// The SSV operator public key (base64 encoded in JSON)
    #[serde(rename = "SSVOperatorPubKey")]
    #[serde(deserialize_with = "deserialize_base64_to_bytes")]
    pub ssv_operator_pub_key: Vec<u8>,
}

impl Operator {
    /// Create a new operator with the given parameters
    pub fn new(operator_id: OperatorId, ssv_operator_pub_key: Vec<u8>) -> Self {
        Self {
            operator_id,
            ssv_operator_pub_key,
        }
    }

    /// Get the operator ID
    pub fn id(&self) -> OperatorId {
        self.operator_id
    }

    /// Get the public key
    pub fn public_key(&self) -> &[u8] {
        &self.ssv_operator_pub_key
    }
}

/// QBFT test configuration with enhanced functionality
///
/// This structure extends the basic TestConfig with additional test-specific
/// parameters and utility methods for QBFT spec test scenarios.
#[derive(Debug, Clone)]
pub struct QbftTestConfig {
    /// Base test configuration
    pub base_config: TestConfig,
    
    /// Committee member information
    pub committee_member: Option<CommitteeMember>,
    
    /// Custom operator ID for testing
    pub test_operator_id: Option<OperatorId>,
    
    /// Custom message identifier for testing
    pub test_identifier: Option<String>,
    
    /// Enable debug logging for this test
    pub debug_enabled: bool,
    
    /// Custom timeout configuration
    pub timeout_config: Option<TimeoutConfig>,
}

/// Timeout configuration for QBFT tests
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Base timeout in milliseconds
    pub base_timeout_ms: u64,
    
    /// Timeout multiplier for each round
    pub round_multiplier: f64,
    
    /// Maximum timeout in milliseconds
    pub max_timeout_ms: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            base_timeout_ms: 1000,
            round_multiplier: 2.0,
            max_timeout_ms: 30000,
        }
    }
}

impl QbftTestConfig {
    /// Create a new QBFT test configuration with default values
    pub fn new() -> Self {
        Self {
            base_config: TestConfig::default(),
            committee_member: None,
            test_operator_id: None,
            test_identifier: None,
            debug_enabled: false,
            timeout_config: None,
        }
    }

    /// Create a configuration from a CommitteeMember
    pub fn from_committee_member(committee_member: CommitteeMember) -> Self {
        let committee_size = committee_member.committee_size();
        let quorum_threshold = committee_member.quorum_threshold();
        
        let base_config = TestConfig {
            committee_size,
            quorum_threshold,
            max_rounds: 100,
            instance_height: 0,
        };

        Self {
            base_config,
            committee_member: Some(committee_member),
            test_operator_id: None,
            test_identifier: None,
            debug_enabled: false,
            timeout_config: None,
        }
    }

    /// Create a configuration for a specific committee size
    pub fn for_committee_size(committee_size: usize) -> Self {
        let quorum_threshold = (committee_size * 2 / 3) + 1;
        
        let base_config = TestConfig {
            committee_size,
            quorum_threshold,
            max_rounds: 100,
            instance_height: 0,
        };

        Self {
            base_config,
            committee_member: None,
            test_operator_id: None,
            test_identifier: None,
            debug_enabled: false,
            timeout_config: None,
        }
    }

    /// Set the test operator ID
    pub fn with_operator_id(mut self, operator_id: OperatorId) -> Self {
        self.test_operator_id = Some(operator_id);
        self
    }

    /// Set the test identifier
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.test_identifier = Some(identifier);
        self
    }

    /// Enable debug logging
    pub fn with_debug(mut self) -> Self {
        self.debug_enabled = true;
        self
    }

    /// Set custom timeout configuration
    pub fn with_timeout_config(mut self, timeout_config: TimeoutConfig) -> Self {
        self.timeout_config = Some(timeout_config);
        self
    }

    /// Set the instance height
    pub fn with_instance_height(mut self, height: u64) -> Self {
        self.base_config.instance_height = height;
        self
    }

    /// Set the maximum rounds
    pub fn with_max_rounds(mut self, max_rounds: u64) -> Self {
        self.base_config.max_rounds = max_rounds;
        self
    }

    /// Get the committee size
    pub fn committee_size(&self) -> usize {
        self.base_config.committee_size
    }

    /// Get the quorum threshold
    pub fn quorum_threshold(&self) -> usize {
        self.base_config.quorum_threshold
    }

    /// Get the instance height
    pub fn instance_height(&self) -> u64 {
        self.base_config.instance_height
    }

    /// Get the maximum rounds
    pub fn max_rounds(&self) -> u64 {
        self.base_config.max_rounds
    }

    /// Get the committee set from the committee member
    pub fn get_committee_set(&self) -> Option<IndexSet<OperatorId>> {
        self.committee_member.as_ref().map(|cm| cm.get_committee_set())
    }

    /// Get the effective operator ID for testing
    pub fn get_effective_operator_id(&self) -> OperatorId {
        self.test_operator_id.unwrap_or_else(|| {
            self.committee_member
                .as_ref()
                .map(|cm| cm.operator_id)
                .unwrap_or(OperatorId::from(1))
        })
    }

    /// Check if the configuration is valid
    pub fn validate(&self) -> Result<(), String> {
        if self.base_config.committee_size == 0 {
            return Err("Committee size must be greater than 0".to_string());
        }

        if self.base_config.quorum_threshold == 0 {
            return Err("Quorum threshold must be greater than 0".to_string());
        }

        if self.base_config.quorum_threshold > self.base_config.committee_size {
            return Err("Quorum threshold cannot exceed committee size".to_string());
        }

        if self.base_config.max_rounds == 0 {
            return Err("Max rounds must be greater than 0".to_string());
        }

        // Validate Byzantine fault tolerance constraint: quorum_threshold >= 2f + 1
        let max_faults = (self.base_config.committee_size - 1) / 3;
        let min_quorum = 2 * max_faults + 1;
        if self.base_config.quorum_threshold < min_quorum {
            return Err(format!(
                "Quorum threshold {} is too low for Byzantine fault tolerance. Minimum required: {}",
                self.base_config.quorum_threshold, min_quorum
            ));
        }

        Ok(())
    }
}

impl Default for QbftTestConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// QBFT test scenario configuration
///
/// This structure encapsulates all the information needed to set up a specific
/// test scenario including the current state, justifications, and test parameters.
#[derive(Debug, Clone)]
pub struct QbftTestScenario {
    /// The current round for this scenario
    pub round: Round,
    
    /// The last prepared round (if any)
    pub last_prepared_round: Option<Round>,
    
    /// The last prepared value (if any)
    pub last_prepared_value: Option<Hash256>,
    
    /// Round change justifications for this scenario
    pub round_change_justifications: Vec<SignedSSVMessage>,
    
    /// Prepare justifications for this scenario
    pub prepare_justifications: Vec<SignedSSVMessage>,
    
    /// Test configuration for this scenario
    pub test_config: QbftTestConfig,
    
    /// Custom test data for this scenario
    pub test_data: Option<Vec<u8>>,
    
    /// Expected outcome for this scenario
    pub expected_outcome: Option<ScenarioOutcome>,
}

/// Expected outcome for a test scenario
#[derive(Debug, Clone)]
pub enum ScenarioOutcome {
    /// Consensus should be reached with the given value
    Consensus(Hash256),
    
    /// The scenario should timeout without reaching consensus
    Timeout,
    
    /// The scenario should produce an error
    Error(String),
    
    /// The scenario should advance to a specific round
    AdvanceToRound(Round),
}

impl QbftTestScenario {
    /// Create a new test scenario with minimal configuration
    pub fn new(round: Round) -> Self {
        Self {
            round,
            last_prepared_round: None,
            last_prepared_value: None,
            round_change_justifications: Vec::new(),
            prepare_justifications: Vec::new(),
            test_config: QbftTestConfig::new(),
            test_data: None,
            expected_outcome: None,
        }
    }

    /// Create a scenario with prepared state
    pub fn with_prepared_state(
        round: Round,
        prepared_round: Round,
        prepared_value: Hash256,
    ) -> Self {
        Self {
            round,
            last_prepared_round: Some(prepared_round),
            last_prepared_value: Some(prepared_value),
            round_change_justifications: Vec::new(),
            prepare_justifications: Vec::new(),
            test_config: QbftTestConfig::new(),
            test_data: None,
            expected_outcome: None,
        }
    }

    /// Set the test configuration
    pub fn with_config(mut self, config: QbftTestConfig) -> Self {
        self.test_config = config;
        self
    }

    /// Add round change justifications
    pub fn with_round_change_justifications(mut self, justifications: Vec<SignedSSVMessage>) -> Self {
        self.round_change_justifications = justifications;
        self
    }

    /// Add prepare justifications
    pub fn with_prepare_justifications(mut self, justifications: Vec<SignedSSVMessage>) -> Self {
        self.prepare_justifications = justifications;
        self
    }

    /// Set test data for this scenario
    pub fn with_test_data(mut self, data: Vec<u8>) -> Self {
        self.test_data = Some(data);
        self
    }

    /// Set expected outcome
    pub fn with_expected_outcome(mut self, outcome: ScenarioOutcome) -> Self {
        self.expected_outcome = Some(outcome);
        self
    }

    /// Get the current round
    pub fn current_round(&self) -> Round {
        self.round
    }

    /// Check if this scenario has prepared state
    pub fn has_prepared_state(&self) -> bool {
        self.last_prepared_round.is_some() && self.last_prepared_value.is_some()
    }

    /// Get the prepared state
    pub fn prepared_state(&self) -> Option<(Round, Hash256)> {
        match (self.last_prepared_round, self.last_prepared_value) {
            (Some(round), Some(value)) => Some((round, value)),
            _ => None,
        }
    }

    /// Check if this scenario has justifications
    pub fn has_justifications(&self) -> bool {
        !self.round_change_justifications.is_empty() || !self.prepare_justifications.is_empty()
    }

    /// Get the total number of justifications
    pub fn justification_count(&self) -> usize {
        self.round_change_justifications.len() + self.prepare_justifications.len()
    }

    /// Check if this scenario expects consensus
    pub fn expects_consensus(&self) -> bool {
        matches!(self.expected_outcome, Some(ScenarioOutcome::Consensus(_)))
    }

    /// Check if this scenario expects timeout
    pub fn expects_timeout(&self) -> bool {
        matches!(self.expected_outcome, Some(ScenarioOutcome::Timeout))
    }

    /// Check if this scenario expects an error
    pub fn expects_error(&self) -> bool {
        matches!(self.expected_outcome, Some(ScenarioOutcome::Error(_)))
    }

    /// Get the expected consensus value (if any)
    pub fn expected_consensus_value(&self) -> Option<Hash256> {
        match &self.expected_outcome {
            Some(ScenarioOutcome::Consensus(value)) => Some(*value),
            _ => None,
        }
    }

    /// Validate the scenario configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate the test configuration
        self.test_config.validate()?;

        // Validate prepared state consistency
        if let Some((prepared_round, _)) = self.prepared_state() {
            if prepared_round >= self.round {
                return Err(format!(
                    "Prepared round {} cannot be >= current round {}",
                    prepared_round, self.round
                ));
            }
        }

        // Validate justifications format
        for (i, justification) in self.round_change_justifications.iter().enumerate() {
            if justification.signatures().is_empty() {
                return Err(format!("Round change justification {} has no signatures", i));
            }
            if justification.operator_ids().is_empty() {
                return Err(format!("Round change justification {} has no operator IDs", i));
            }
        }

        for (i, justification) in self.prepare_justifications.iter().enumerate() {
            if justification.signatures().is_empty() {
                return Err(format!("Prepare justification {} has no signatures", i));
            }
            if justification.operator_ids().is_empty() {
                return Err(format!("Prepare justification {} has no operator IDs", i));
            }
        }

        Ok(())
    }
}

/// Utility functions for working with QBFT test types
impl QbftTestScenario {
    /// Create a basic scenario for testing message creation
    pub fn for_message_creation(round: Round, committee_size: usize) -> Self {
        let config = QbftTestConfig::for_committee_size(committee_size);
        Self::new(round).with_config(config)
    }

    /// Create a scenario for testing round changes
    pub fn for_round_change(
        _from_round: Round,
        to_round: Round,
        committee_size: usize,
    ) -> Self {
        let config = QbftTestConfig::for_committee_size(committee_size);
        Self::new(to_round)
            .with_config(config)
            .with_expected_outcome(ScenarioOutcome::AdvanceToRound(to_round))
    }

    /// Create a scenario for testing consensus
    pub fn for_consensus(
        round: Round,
        expected_value: Hash256,
        committee_size: usize,
    ) -> Self {
        let config = QbftTestConfig::for_committee_size(committee_size);
        Self::new(round)
            .with_config(config)
            .with_expected_outcome(ScenarioOutcome::Consensus(expected_value))
    }

    /// Create a scenario for testing timeout behavior
    pub fn for_timeout(round: Round, committee_size: usize) -> Self {
        let config = QbftTestConfig::for_committee_size(committee_size);
        Self::new(round)
            .with_config(config)
            .with_expected_outcome(ScenarioOutcome::Timeout)
    }
}

/// Example usage of the common types for QBFT testing
pub mod examples {
    use super::*;
    
    /// Example: Create a basic 4-node committee configuration
    pub fn create_four_node_committee() -> CommitteeMember {
        let operators = vec![
            Operator::new(OperatorId::from(1), vec![1, 2, 3, 4]),
            Operator::new(OperatorId::from(2), vec![5, 6, 7, 8]),
            Operator::new(OperatorId::from(3), vec![9, 10, 11, 12]),
            Operator::new(OperatorId::from(4), vec![13, 14, 15, 16]),
        ];
        
        CommitteeMember::new(
            OperatorId::from(1),
            b"committee_123".to_vec(),
            b"operator_1_pubkey".to_vec(),
            1, // Can tolerate 1 faulty node
            operators,
            vec![0x01, 0x02, 0x03, 0x04], // Domain type
        )
    }
    
    /// Example: Create a test configuration for message creation tests
    pub fn create_message_test_config() -> QbftTestConfig {
        QbftTestConfig::for_committee_size(4)
            .with_operator_id(OperatorId::from(1))
            .with_debug()
            .with_instance_height(1)
            .with_max_rounds(10)
    }
    
    /// Example: Create a test scenario for consensus testing
    pub fn create_consensus_scenario() -> QbftTestScenario {
        let expected_value = Hash256::random();
        QbftTestScenario::for_consensus(
            Round::from(1),
            expected_value,
            4,
        )
        .with_test_data(b"test_data".to_vec())
    }
    
    /// Example: Create a test scenario with prepared state
    pub fn create_prepared_scenario() -> QbftTestScenario {
        let prepared_value = Hash256::random();
        QbftTestScenario::with_prepared_state(
            Round::from(3),
            Round::from(2),
            prepared_value,
        )
        .with_config(create_message_test_config())
        .with_expected_outcome(ScenarioOutcome::Consensus(prepared_value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_committee_member_creation() {
        let operator1 = Operator::new(OperatorId::from(1), vec![1, 2, 3]);
        let operator2 = Operator::new(OperatorId::from(2), vec![4, 5, 6]);
        let committee = vec![operator1, operator2];

        let committee_member = CommitteeMember::new(
            OperatorId::from(1),
            vec![1, 2, 3, 4],
            vec![5, 6, 7, 8],
            1,
            committee,
            vec![9, 10, 11, 12],
        );

        assert_eq!(committee_member.operator_id, OperatorId::from(1));
        assert_eq!(committee_member.committee_size(), 2);
        assert_eq!(committee_member.quorum_threshold(), 2);
        assert!(committee_member.contains_operator(OperatorId::from(1)));
        assert!(committee_member.contains_operator(OperatorId::from(2)));
        assert!(!committee_member.contains_operator(OperatorId::from(3)));
    }

    #[test]
    fn test_qbft_test_config() {
        let config = QbftTestConfig::for_committee_size(4)
            .with_operator_id(OperatorId::from(1))
            .with_debug()
            .with_instance_height(10)
            .with_max_rounds(50);

        assert_eq!(config.committee_size(), 4);
        assert_eq!(config.quorum_threshold(), 3);
        assert_eq!(config.instance_height(), 10);
        assert_eq!(config.max_rounds(), 50);
        assert_eq!(config.get_effective_operator_id(), OperatorId::from(1));
        assert!(config.debug_enabled);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_qbft_test_scenario() {
        let round = Round::from(2);
        let prepared_round = Round::from(1);
        let prepared_value = Hash256::random();
        let committee_size = 4;

        let scenario = QbftTestScenario::with_prepared_state(round, prepared_round, prepared_value)
            .with_config(QbftTestConfig::for_committee_size(committee_size))
            .with_expected_outcome(ScenarioOutcome::Consensus(prepared_value));

        assert_eq!(scenario.current_round(), round);
        assert!(scenario.has_prepared_state());
        assert!(scenario.expects_consensus());
        assert_eq!(scenario.expected_consensus_value(), Some(prepared_value));
        assert!(scenario.validate().is_ok());
    }

    #[test]
    fn test_scenario_validation() {
        let round = Round::from(1);
        let prepared_round = Round::from(2); // Invalid: prepared round >= current round
        let prepared_value = Hash256::random();

        let scenario = QbftTestScenario::with_prepared_state(round, prepared_round, prepared_value);
        assert!(scenario.validate().is_err());
    }

    #[test]
    fn test_quorum_threshold_calculation() {
        // Test various committee sizes
        assert_eq!(QbftTestConfig::for_committee_size(1).quorum_threshold(), 1);
        assert_eq!(QbftTestConfig::for_committee_size(2).quorum_threshold(), 2);
        assert_eq!(QbftTestConfig::for_committee_size(3).quorum_threshold(), 3);
        assert_eq!(QbftTestConfig::for_committee_size(4).quorum_threshold(), 3);
        assert_eq!(QbftTestConfig::for_committee_size(5).quorum_threshold(), 4);
        assert_eq!(QbftTestConfig::for_committee_size(6).quorum_threshold(), 5);
        assert_eq!(QbftTestConfig::for_committee_size(7).quorum_threshold(), 5);
    }

    #[test]
    fn test_byzantine_fault_tolerance_validation() {
        // Test that the configuration validates Byzantine fault tolerance
        let config = QbftTestConfig::for_committee_size(4); // Should allow 1 fault
        assert!(config.validate().is_ok());

        // Test invalid configuration
        let mut invalid_config = QbftTestConfig::new();
        invalid_config.base_config.committee_size = 4;
        invalid_config.base_config.quorum_threshold = 2; // Too low for BFT
        assert!(invalid_config.validate().is_err());
    }
}