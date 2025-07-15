use base64::Engine;
use openssl::pkey::{PKey, Private};
use qbft::TestConfig;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use ssv_types::{IndexSet, OperatorId, Round, consensus::QbftMessageType, msgid::MessageId};
use tree_hash::TreeHash;
use types::Hash256;

use super::{
    qbft_deserializers::*,
    unified_test_adapter::{TestScenario, UnifiedTestAdapter},
};
use crate::{QbftSpecTestType, SpecTest, SpecTestType, utils::test_keys::TestKeySet};
use ssv_types::message::SignedSSVMessage;

impl SpecTest for CreateMessageTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self) -> bool {
        // Validate test data before running
        if let Err(e) = self.validate_justification_data() {
            eprintln!("Validation failed: {}", e);
            return false;
        }

        // Set up UnifiedTestAdapter
        let mut unified_adapter = match self.create_qbft_instance() {
            Ok(adapter) => adapter,
            Err(e) => {
                eprintln!("Adapter setup failed: {}", e);
                return false;
            }
        };

        // Configure test scenario
        if let Err(e) = self.setup_test_scenario(&mut unified_adapter) {
            eprintln!("Scenario setup failed: {}", e);
            return false;
        }

        // Create and verify message
        match self.create_and_verify_message(&mut unified_adapter) {
            Ok(success) => success,
            Err(e) => {
                eprintln!("Message creation failed: {}", e);
                false
            }
        }
    }

    // Setup the qbft instance for constructing a new message
    fn setup(&mut self) {
        // Setup is now handled in run() method since we need mutable access
        // This method is kept for compatibility with the test framework
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::CreateMessage)
    }
}

// Committee member structure from JSON
#[derive(Deserialize)]
pub struct CommitteeMember {
    #[serde(rename = "OperatorID")]
    pub operator_id: u64,

    #[serde(rename = "CommitteeID")]
    pub committee_id: Vec<u8>,

    #[serde(rename = "SSVOperatorPubKey")]
    pub ssv_operator_pub_key: String,

    #[serde(rename = "FaultyNodes")]
    pub faulty_nodes: u64,

    #[serde(rename = "Committee")]
    pub committee: Vec<Operator>,

    #[serde(rename = "DomainType")]
    pub domain_type: [u8; 4],
}

// Operator structure from JSON
#[derive(Deserialize)]
pub struct Operator {
    #[serde(rename = "OperatorID")]
    pub operator_id: u64,

    #[serde(rename = "SSVOperatorPubKey")]
    pub ssv_operator_pub_key: String,
}

// Representation of CreateMsgSpecTest files
#[derive(Deserialize)]
pub struct CreateMessageTest {
    // Name of the test that is being run
    #[serde(rename = "Name")]
    pub name: String,

    // Root of the QBFT Message, This is the unhashed ssz bytes of the data
    #[serde(rename = "Value", deserialize_with = "deserialize_value_into_root")]
    pub root: Hash256,

    // The last prepared value of the qbft instance. Todo!() What format is this in??
    #[serde(rename = "StateValue")]
    pub state_value: Option<String>,

    // The round this message is for
    #[serde(rename = "Round", deserialize_with = "deserialize_u64_into_round")]
    pub round: Option<Round>,

    // Any round change justifications for the message
    #[serde(rename = "RoundChangeJustifications")]
    pub round_change_justifications: Option<Vec<SignedSSVMessage>>,

    // Any prepare justifications for the message
    #[serde(rename = "PrepareJustifications")]
    pub prepare_justifications: Option<Vec<SignedSSVMessage>>,

    // The type of the QBFT Message to create
    #[serde(
        rename = "CreateType",
        deserialize_with = "deserialize_qbft_message_type"
    )]
    pub create_type: QbftMessageType,

    // The Expected Root of the QBFT Message
    #[serde(rename = "ExpectedRoot")]
    pub expected_root: Hash256,

    // Any Errors that were expected
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,

    // Unified test adapter that is used for running the test. Skip this during deserialization
    #[serde(skip)]
    pub unified_adapter: Option<UnifiedTestAdapter>,

    // New fields from hardcoded test data
    #[serde(rename = "Type")]
    pub test_type: Option<String>,

    #[serde(rename = "Documentation")]
    pub documentation: Option<String>,

    #[serde(rename = "CommitteeMember")]
    pub committee_member: Option<CommitteeMember>,

    #[serde(rename = "Identifier")]
    pub identifier: Option<String>,

    #[serde(rename = "OperatorID")]
    pub operator_id: Option<u64>,

    // The operator private key for message signing
    #[serde(skip)]
    pub signing_key: Option<PKey<Private>>,
}

impl CreateMessageTest {
    /// Create and configure a UnifiedTestAdapter for testing
    fn create_qbft_instance(&self) -> Result<UnifiedTestAdapter, String> {
        // Use committee data from JSON if available, otherwise fallback to test keys
        let (committee, identifier, private_key) =
            if let Some(committee_member) = &self.committee_member {
                // Parse committee from JSON
                let committee: IndexSet<OperatorId> = committee_member
                    .committee
                    .iter()
                    .map(|op| OperatorId::from(op.operator_id))
                    .collect();

                // Use identifier from JSON if available
                let identifier = if let Some(id_str) = &self.identifier {
                    self.parse_message_id_from_base64(id_str)?
                } else {
                    MessageId::for_spectest()
                };

                // Use operator ID from JSON if available
                let operator_id = self.operator_id.unwrap_or(1);

                // Parse the RSA key for the operator
                let operator_key_data = committee_member
                    .committee
                    .iter()
                    .find(|op| op.operator_id == operator_id)
                    .ok_or("Operator not found in committee")?;

                let private_key =
                    self.parse_rsa_key_from_base64(&operator_key_data.ssv_operator_pub_key)?;

                (committee, identifier, private_key)
            } else {
                // Fallback to existing behavior
                let four_share_set = TestKeySet::four_share_set();
                let committee: IndexSet<OperatorId> =
                    four_share_set.operator_keys.keys().cloned().collect();
                let identifier = MessageId::for_spectest();

                let operator_key = four_share_set
                    .operator_keys
                    .get(&OperatorId::from(1))
                    .ok_or("Operator key not found")?;

                let private_key = PKey::from_rsa(operator_key.to_owned())
                    .map_err(|e| format!("Failed to create private key: {}", e))?;

                (committee, identifier, private_key)
            };

        let config = TestConfig {
            committee_size: committee.len(),
            quorum_threshold: (committee.len() * 2 / 3) + 1,
            max_rounds: 100,
            instance_height: 0,
        };

        let mut unified_adapter = UnifiedTestAdapter::new(committee, identifier, config)
            .map_err(|e| format!("Failed to create unified adapter: {}", e))?;
        unified_adapter.set_signing_key(private_key);

        Ok(unified_adapter)
    }

    /// Parse base64-encoded MessageId from JSON
    fn parse_message_id_from_base64(&self, id_str: &str) -> Result<MessageId, String> {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(id_str)
            .map_err(|e| format!("Failed to decode base64 identifier: {}", e))?;

        if decoded.len() != 56 {
            return Err(format!(
                "Invalid identifier length: expected 56, got {}",
                decoded.len()
            ));
        }

        let mut id_bytes = [0u8; 56];
        id_bytes.copy_from_slice(&decoded);
        Ok(MessageId::from(id_bytes))
    }

    /// Parse base64-encoded RSA key from JSON - for testing, we use the corresponding private key
    fn parse_rsa_key_from_base64(&self, _key_str: &str) -> Result<PKey<Private>, String> {
        // The JSON contains base64-encoded public keys, but we need private keys for signing
        // For testing purposes, we'll use the existing test private keys that correspond
        // to the public keys in the JSON test data
        let four_share_set = TestKeySet::four_share_set();
        let operator_id = self.operator_id.unwrap_or(1);
        let operator_key = four_share_set
            .operator_keys
            .get(&OperatorId::from(operator_id))
            .ok_or("Operator key not found in test set")?;

        PKey::from_rsa(operator_key.to_owned())
            .map_err(|e| format!("Failed to create private key: {}", e))
    }

    /// Set up the test scenario with proper state and justifications
    fn setup_test_scenario(&self, unified_adapter: &mut UnifiedTestAdapter) -> Result<(), String> {
        let (last_prepared_round, last_prepared_value) = self.extract_prepared_state()?;

        let scenario = TestScenario {
            round: self.round.unwrap_or(1.into()),
            last_prepared_round,
            last_prepared_value,
            round_change_justifications: self
                .round_change_justifications
                .clone()
                .unwrap_or_default(),
            prepare_justifications: self.prepare_justifications.clone().unwrap_or_default(),
        };

        unified_adapter
            .setup_test_scenario(scenario)
            .map_err(|e| format!("Scenario setup error: {}", e))
    }

    /// Extract prepared state from test data
    fn extract_prepared_state(&self) -> Result<(Option<Round>, Option<Hash256>), String> {
        let Some(state_value) = &self.state_value else {
            return Ok((None, None));
        };

        let decoded_state_value = base64::engine::general_purpose::STANDARD
            .decode(state_value)
            .map_err(|e| format!("Failed to decode state value: {}", e))?;

        let state_value_hash = Hash256::from_slice(&Sha256::digest(&decoded_state_value));
        Ok((Some(self.round.unwrap_or(1.into())), Some(state_value_hash)))
    }

    /// Create message and verify its root
    fn create_and_verify_message(
        &self,
        unified_adapter: &mut UnifiedTestAdapter,
    ) -> Result<bool, String> {
        let state_value = self
            .state_value
            .as_ref()
            .and_then(|sv| base64::engine::general_purpose::STANDARD.decode(sv).ok());

        let round_change_justifications = self
            .round_change_justifications
            .as_deref()
            .unwrap_or(&[])
            .to_vec();
        let prepare_justifications = self
            .prepare_justifications
            .as_deref()
            .unwrap_or(&[])
            .to_vec();

        let signed_message = unified_adapter
            .create_message(
                self.create_type,
                self.root,
                self.round,
                state_value,
                round_change_justifications,
                prepare_justifications,
            )
            .map_err(|e| format!("Message creation failed: {}", e))?;

        let root_matches = unified_adapter.verify_root(signed_message.clone(), self.expected_root);

        if !root_matches {
            self.log_verification_failure(&signed_message);
        }

        Ok(root_matches)
    }

    /// Log detailed information about root verification failures
    fn log_verification_failure(&self, signed_message: &SignedSSVMessage) {
        eprintln!("Root verification failed for test: {}", self.name);
        eprintln!("Expected root: {:?}", self.expected_root);
        eprintln!("Actual root: {:?}", signed_message.tree_hash_root());

        // Additional debugging for specific test types
        if self.name.contains("create proposal") && !self.name.contains("previously prepared") {
            eprintln!("Message details for proposal test:");
            eprintln!("  SSV Message: {:?}", signed_message.ssv_message());
            eprintln!("  Full data length: {}", signed_message.full_data().len());
            eprintln!("  Signatures: {:?}", signed_message.signatures());
            eprintln!("  Operator IDs: {:?}", signed_message.operator_ids());
        }
    }

    /// Validate that justifications have expected format
    fn validate_justification_data(&self) -> Result<(), String> {
        // Validate round change justifications
        if let Some(ref round_change_justifications) = self.round_change_justifications {
            for (i, justification) in round_change_justifications.iter().enumerate() {
                if justification.signatures().is_empty() {
                    return Err(format!("Round change justification {i} has no signatures"));
                }
                if justification.operator_ids().is_empty() {
                    return Err(format!(
                        "Round change justification {i} has no operator IDs"
                    ));
                }

                // Check if full_data is base64 decodable (if not empty)
                if !justification.full_data().is_empty() {
                    let full_data = justification.full_data();
                    if full_data.len() > 100 {
                        // Reasonable max size check
                        return Err(format!(
                            "Round change justification {i} has unexpectedly large full_data: {} bytes",
                            full_data.len()
                        ));
                    }
                }
            }
        }

        // Validate prepare justifications
        if let Some(ref prepare_justifications) = self.prepare_justifications {
            for (i, justification) in prepare_justifications.iter().enumerate() {
                if justification.signatures().is_empty() {
                    return Err(format!("Prepare justification {i} has no signatures"));
                }
                if justification.operator_ids().is_empty() {
                    return Err(format!("Prepare justification {i} has no operator IDs"));
                }

                // Check if full_data is reasonable
                if !justification.full_data().is_empty() {
                    let full_data = justification.full_data();
                    if full_data.len() > 100 {
                        // Reasonable max size check
                        return Err(format!(
                            "Prepare justification {i} has unexpectedly large full_data: {} bytes",
                            full_data.len()
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}
