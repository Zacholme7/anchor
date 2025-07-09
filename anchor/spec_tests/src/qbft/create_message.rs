use openssl::pkey::{PKey, Private};
use serde::Deserialize;
use ssv_types::{IndexSet, OperatorId, Round, consensus::QbftMessageType, msgid::MessageId};
use types::Hash256;
use base64::Engine;
use tree_hash::TreeHash;
use sha2::{Sha256, Digest};

use super::{SpecQbft, qbft_deserializers::*};
use crate::{
    QbftSpecTestType, SpecTest, SpecTestType, qbft::SignedSSVMessage, utils::test_keys::TestKeySet,
};
use qbft::test_adapter::TestScenario;

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

        // Set up QBFT instance
        let mut spec_qbft = match self.create_qbft_instance() {
            Ok(qbft) => qbft,
            Err(e) => {
                eprintln!("QBFT setup failed: {}", e);
                return false;
            }
        };

        // Configure test scenario
        if let Err(e) = self.setup_test_scenario(&mut spec_qbft) {
            eprintln!("Scenario setup failed: {}", e);
            return false;
        }

        // Create and verify message
        match self.create_and_verify_message(&mut spec_qbft) {
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

// Representation of CreateMsgSpecTest files
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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

    // Qbft Instance that is used for running the test. Skip this during deserialization
    #[serde(skip)]
    pub spec_qbft: Option<SpecQbft>,

    // The operator private key for message signing
    #[serde(skip)]
    pub signing_key: Option<PKey<Private>>,
}

impl CreateMessageTest {
    /// Create and configure a QBFT instance for testing
    fn create_qbft_instance(&self) -> Result<SpecQbft, String> {
        let four_share_set = TestKeySet::four_share_set();
        let committee: IndexSet<OperatorId> = four_share_set.operator_keys.keys().cloned().collect();
        let identifier = MessageId::for_spectest();
        
        let operator_key = four_share_set
            .operator_keys
            .get(&OperatorId::from(1))
            .ok_or("Operator key not found")?;
            
        let private_key = PKey::from_rsa(operator_key.to_owned())
            .map_err(|e| format!("Failed to create private key: {}", e))?;

        let mut spec_qbft = SpecQbft::new(committee, identifier);
        spec_qbft.set_signing_key(private_key);
        
        Ok(spec_qbft)
    }

    /// Set up the test scenario with proper state and justifications
    fn setup_test_scenario(&self, spec_qbft: &mut SpecQbft) -> Result<(), String> {
        let (last_prepared_round, last_prepared_value) = self.extract_prepared_state()?;
        
        let scenario = TestScenario {
            round: self.round.unwrap_or(1.into()),
            last_prepared_round,
            last_prepared_value,
            round_change_justifications: Vec::new(), // Don't set in containers
            prepare_justifications: Vec::new(),       // Don't set in containers
        };
        
        spec_qbft.setup_test_scenario(scenario)
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
    fn create_and_verify_message(&self, spec_qbft: &mut SpecQbft) -> Result<bool, String> {
        let state_value = self.state_value.as_ref()
            .and_then(|sv| base64::engine::general_purpose::STANDARD.decode(sv).ok());
            
        let round_change_justifications = self.round_change_justifications.as_deref().unwrap_or(&[]).to_vec();
        let prepare_justifications = self.prepare_justifications.as_deref().unwrap_or(&[]).to_vec();

        let signed_message = spec_qbft
            .create_message(
                self.create_type,
                self.root,
                self.round,
                state_value,
                round_change_justifications,
                prepare_justifications,
            )
            .map_err(|e| format!("Message creation failed: {}", e))?;

        let root_matches = spec_qbft.verify_root(signed_message.clone(), self.expected_root);
        
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
