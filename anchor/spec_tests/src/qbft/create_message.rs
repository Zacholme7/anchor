use super::adapter::{
    MessageCreationRequest, QbftTestAdapter, SpecTestCommitteeMember, TestContext, TestType,
};
use crate::utils::deserializers::qbft_deserializers::deserialize_qbft_message_type;
use crate::{QbftSpecTestType, SpecTest, SpecTestType};
use base64;
use serde::Deserialize;
use ssv_types::{
    Round,
    consensus::{QbftMessage, QbftMessageType},
    message::{SSVMessage, SignedSSVMessage},
};
use ssz::Decode;
use tree_hash::TreeHash;
use types::Hash256;

#[derive(Debug, Clone, Deserialize)]
pub struct CreateMessageTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(
        rename = "CreateType",
        deserialize_with = "deserialize_qbft_message_type"
    )]
    pub msg_type: QbftMessageType,
    #[serde(rename = "Value")]
    pub data_hash: Vec<u8>, // JSON has this as an array, we'll convert to Hash256
    #[serde(rename = "Round")]
    pub round: Option<u64>,
    #[serde(rename = "StateValue")]
    pub state_value: Option<String>,
    #[serde(rename = "RoundChangeJustifications")]
    pub round_change_justifications: Option<Vec<SignedSSVMessage>>,
    #[serde(rename = "PrepareJustifications")]
    pub prepare_justifications: Option<Vec<SignedSSVMessage>>,
    #[serde(rename = "ExpectedRoot")]
    pub expected_root: Hash256,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
    #[serde(rename = "OperatorID")]
    pub operator_id: u64,
    #[serde(rename = "CommitteeMember")]
    pub committee_member: SpecTestCommitteeMember,
    #[serde(rename = "Identifier")]
    pub identifier: Option<String>,
}

impl SpecTest for CreateMessageTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self) -> bool {


        // Create test context
        let test_context = TestContext::new(self.name.clone(), TestType::MessageCreation)
            .with_expected_errors(vec![self.expected_error.clone()]);

        // Create adapter for message creation
        let mut adapter = match QbftTestAdapter::with_default_committee() {
            Ok(adapter) => adapter.with_test_context(test_context),
            Err(_) => return false,
        };

        // Setup scenario if needed
        let prepare_justifications = self.prepare_justifications.clone().unwrap_or_default();

        if let Err(e) = adapter.setup_message_creation_scenario(
            self.round.map(Round::from),
            self.state_value.clone(),
            self.round_change_justifications.clone().unwrap_or_default(),
            prepare_justifications,
        ) {
            if self.is_expected_error(&e.to_string()) {
                return true;
            } else {
                return false;
            }
        }

        // Decode the base64 identifier from the JSON test
        let identifier_bytes = if let Some(ref identifier_b64) = self.identifier {
            use base64::{Engine as _, engine::general_purpose};
            match general_purpose::STANDARD.decode(identifier_b64) {
                Ok(bytes) => Some(bytes),
                Err(_) => {
                    return false;
                }
            }
        } else {
            None
        };

        // Execute message creation scenario with the correct identifier
        let request = self.create_message_request();
        let scenario_result = adapter.execute_message_creation_scenario_with_committee_id(
            request,
            self.expected_root,
            identifier_bytes,
        );


        // Assert result
        self.assert_message_creation_result(&scenario_result)
    }

    fn setup(&mut self) {
        // No setup needed for message creation tests
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::CreateMessage)
    }
}

impl CreateMessageTest {
    /// Convert Vec<u8> data hash to Hash256
    fn get_data_hash(&self) -> Hash256 {
        let mut hash_bytes = [0u8; 32];
        let copy_len = std::cmp::min(self.data_hash.len(), 32);
        hash_bytes[..copy_len].copy_from_slice(&self.data_hash[..copy_len]);
        Hash256::from(hash_bytes)
    }

    /// Create message creation request from test data
    fn create_message_request(&self) -> MessageCreationRequest {
        let prepare_justifications = self.prepare_justifications.clone().unwrap_or_default();


        MessageCreationRequest {
            msg_type: self.msg_type,
            data_hash: self.get_data_hash(),
            round: self.round.map(Round::from),
            state_value: self.state_value.as_ref().and_then(|s| {
                use base64::{Engine as _, engine::general_purpose};
                general_purpose::STANDARD.decode(s).ok()
            }),
            round_change_justifications: self
                .round_change_justifications
                .clone()
                .unwrap_or_default(),
            prepare_justifications,
        }
    }

    /// Assert message creation result
    fn assert_message_creation_result(&self, result: &super::adapter::ScenarioResult) -> bool {

        // Check for expected errors first
        if !self.expected_error.is_empty() {
            if result
                .go_formatted_errors
                .iter()
                .any(|err| err.contains(&self.expected_error))
            {
                return true;
            } else {
                return false;
            }
        }

        // Check if message was created successfully
        if result.processing_result.messages_sent.is_empty() {
            return false;
        }

        let created_message = &result.processing_result.messages_sent[0];



        // Validate root hash if expected
        let actual_root = created_message.tree_hash_root();



        if actual_root != self.expected_root {
            return false;
        }

        // Check validation result
        if !result.processing_result.validation_result.is_valid {
            return false;
        }

        true
    }

    /// Check if error message matches expected error
    fn is_expected_error(&self, error: &str) -> bool {
        !self.expected_error.is_empty() && error.contains(&self.expected_error)
    }
}

/// Helper function to decode QbftMessage from SSVMessage data
fn decode_qbft_message_from_ssv(ssv_message: &SSVMessage) -> Result<QbftMessage, String> {
    // The SSVMessage.data contains the encoded QbftMessage
    let data = ssv_message.data();

    // Try to decode the QbftMessage from the data
    QbftMessage::from_ssz_bytes(data).map_err(|e| format!("Failed to decode QbftMessage: {:?}", e))
}
