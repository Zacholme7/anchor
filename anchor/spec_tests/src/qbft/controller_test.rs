use super::adapters::spec_types::SpecTestCommitteeMember;
use super::common_types::ExpectedTimerState;
use crate::types::TestSignedSSVMessage;
use crate::utils::deserializers::{
    deserialize_base64, deserialize_base64_option, deserialize_hex_hash256_option,
};
use crate::{QbftSpecTestType, SpecTest, SpecTestType};
use serde::Deserialize;
use types::Hash256;

#[derive(Debug, Clone, Deserialize)]
pub struct TestController {
    #[serde(rename = "Identifier")]
    #[serde(deserialize_with = "deserialize_base64")]
    pub identifier: Vec<u8>,
    #[serde(rename = "Height")]
    pub height: u64,
    #[serde(rename = "StoredInstances")]
    pub stored_instances: Vec<serde_json::Value>, // Can be empty
    #[serde(rename = "CommitteeMember")]
    pub committee_member: SpecTestCommitteeMember,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ControllerTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(rename = "RunInstanceData")]
    pub run_instance_data: Vec<RunInstanceData>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
    #[serde(rename = "Controller")]
    pub controller: Option<TestController>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunInstanceData {
    #[serde(rename = "Height")]
    pub height: Option<u64>,
    #[serde(rename = "InputValue")]
    #[serde(deserialize_with = "deserialize_base64_option")]
    pub input_value: Option<Vec<u8>>,
    #[serde(rename = "InputMessages")]
    pub input_messages: Option<Vec<TestSignedSSVMessage>>,
    #[serde(rename = "ControllerPostRoot")]
    #[serde(deserialize_with = "deserialize_hex_hash256_option")]
    pub controller_post_root: Option<Hash256>,
    #[serde(rename = "ExpectedDecidedState")]
    pub expected_decided_state: Option<ExpectedDecidedState>,
    #[serde(rename = "ExpectedTimerState")]
    pub expected_timer_state: Option<ExpectedTimerState>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedDecidedState {
    #[serde(rename = "DecidedCnt")]
    pub decided_count: u64,
    #[serde(rename = "DecidedVal")]
    #[serde(deserialize_with = "deserialize_base64_option")]
    pub decided_value: Option<Vec<u8>>,
}

impl SpecTest for ControllerTest {
    fn run(&self) -> bool {
        // TODO: Implement controller test logic
        true
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::Controller)
    }
}
