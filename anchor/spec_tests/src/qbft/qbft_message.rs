use super::adapter::{QbftTestAdapter, TestContext, TestType};
use crate::qbft::adapter::error_mapping::map_signed_ssv_error_to_go_format;
use crate::{QbftSpecTestType, SpecTest, SpecTestType, types::TestSignedSSVMessage};
use base64::prelude::*;
use serde::Deserialize;
use ssv_types::consensus::QbftMessage;
use ssv_types::message::{SignedSSVMessage, SignedSSVMessageError};
use ssz::{Decode, Encode};
use tree_hash::TreeHash;

#[derive(Deserialize)]
pub struct QbftMessageTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(rename = "Messages")]
    pub messages: Vec<SignedSSVMessage>,
    #[serde(rename = "EncodedMessages")]
    pub encoded_messages: Vec<Vex<u8>>,
    #[serde(rename = "EncodedRoots")]
    pub expected_roots: Vec<Vex<Hash256>>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
}

impl SpecTest for QbftMessageTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) {}

    fn run(&self) -> bool {
        let mut last_error: Option<SignedSSVMessageError> = None;

        for (i, message) in self.messages.iter().enumerate() {
            if let Err(e) = message.validate() {
                last_error = e;
                continue;
            }

            let qbft_message = match QbftMessage::from_ssz_bytes(message.full_data()) {
                Ok(msg) => msg,
                Err(e) => {
                    last_error = e;
                    continue;
                }
            };

            if !self.encoded_messages.is_empty() {
                let encoded = message.as_ssz_bytes();
                if self.encoded_messages[i] != encoded {
                    return false;
                }
            }

            if !self.expected_roots.is_empty() {
                let root = message.tree_hash_root();
                if self.expected_roots[i] != root {
                    return false;
                }
            }
        }

        if self.expected_error != String::from("") {
            // have the error mapping here: todo!()
            false
        } else {
            last_error.is_none()
        }
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::QbftMessage)
    }
}
