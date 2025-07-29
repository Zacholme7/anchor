use super::adapter::{
    MessageCreationRequest, QbftTestAdapter, SpecTestCommitteeMember, TestContext, TestType,
};
use super::adapters::qbft::*;
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
    pub root: Vec<u8>, // JSON has this as an array, we'll convert to Hash256
    #[serde(rename = "Round")]
    pub round: Option<u64>,
    #[serde(rename = "StateValue")]
    pub value: Option<String>,
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
    #[serde(skip)]
    qbft_adapter: Option<QbftAdapter>,
}

impl SpecTest for CreateMessageTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) {
        // Build teh state
        let starting_state = QbftStartingState { height: None };
        self.qbft_adapter = Some(QbftAdapter::new_with_state(starting_state));
    }

    fn run(&self) -> bool {
        if let Some(adapter) = &self.qbft_adapter {
            let signed_ssv_message = match adapter.create_message(
                self.msg_type,
                Hash256::default(),
                &self.round_change_justifications,
                &self.prepare_justifications,
                self.round,
            ) {
                Ok(msg) => msg,
                Err(e) => {
                    // check the expected errors
                    todo!()
                }
            };

            // compare the roots
            let root = signed_ssv_message.tree_hash_root();
            if root != self.expected_root {
                return false;
            }

            // Validate the SignedSSVMessage
            if signed_ssv_message.validate().is_err() {
                return false;
            }

            // deser the qbft message and also call validate on that
            let Ok(_qbft_message) = QbftMessage::from_ssz_bytes(signed_ssv_message.full_data())
            else {
                return false;
            };

            // todo!() we dont have this
            //if qbft_message.validate().is_err() {
            //    return false;
            //}

            // State comparison: todo!()
            return true;
        }
        false
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::CreateMessage)
    }
}
