use crate::utils::deserializers::ssv_message_parse::{
    deserialize_base64_or_vec, deserialize_optional_base64_or_vec, deserialize_signature_map,
};
use crate::{SpecTest, SpecTestType, SsvSpecTestType};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ssv_types::message::SignedSSVMessage;
use std::collections::HashMap;

// Wrapper for SignedSSVMessage with custom deserialization
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlexibleSignedSSVMessage {
    #[serde(rename = "Signatures")]
    pub signatures: Vec<String>,
    #[serde(rename = "OperatorIDs")]
    pub operator_ids: Vec<u64>,
    #[serde(rename = "SSVMessage")]
    pub ssv_message: Option<Value>, // Make SSVMessage optional to handle null
    #[serde(rename = "FullData")]
    #[serde(deserialize_with = "deserialize_base64_or_vec")]
    pub full_data: Vec<u8>,
}

impl FlexibleSignedSSVMessage {
    /// Convert to SignedSSVMessage if possible
    pub fn to_signed_ssv_message(&self) -> Result<SignedSSVMessage, String> {
        // For now, we'll try to serialize back to JSON and then deserialize as SignedSSVMessage
        // This handles the base64 conversions and null handling
        let json_value = serde_json::json!({
            "Signatures": self.signatures,
            "OperatorIDs": self.operator_ids,
            "SSVMessage": self.ssv_message,
            "FullData": self.full_data
        });

        serde_json::from_value(json_value)
            .map_err(|e| format!("Failed to convert to SignedSSVMessage: {}", e))
    }

    /// Check if this message is valid (has non-null SSVMessage)
    pub fn is_valid(&self) -> bool {
        self.ssv_message.is_some() && !self.ssv_message.as_ref().unwrap().is_null()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JsonShare {
    #[serde(rename = "ValidatorIndex")]
    pub validator_index: String,
    #[serde(rename = "ValidatorPubKey")]
    pub validator_pub_key: Vec<u8>,
    #[serde(rename = "SharePubKey")]
    pub share_pub_key: String,
    #[serde(rename = "Committee")]
    pub committee: Vec<JsonShareMember>,
    #[serde(rename = "DomainType")]
    pub domain_type: Vec<u8>,
    #[serde(rename = "FeeRecipientAddress")]
    pub fee_recipient_address: Vec<u8>,
    #[serde(rename = "Graffiti")]
    pub graffiti: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JsonShareMember {
    #[serde(rename = "SharePubKey")]
    pub share_pub_key: String,
    #[serde(rename = "Signer")]
    pub signer: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputMessage {
    #[serde(rename = "Type")]
    pub message_type: u64,
    #[serde(rename = "Slot")]
    pub slot: String,
    #[serde(rename = "Messages")]
    pub messages: Vec<OutputMessageData>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputMessageData {
    #[serde(rename = "PartialSignature")]
    pub partial_signature: String,
    #[serde(rename = "SigningRoot")]
    pub signing_root: Vec<u8>,
    #[serde(rename = "Signer")]
    pub signer: u64,
    #[serde(rename = "ValidatorIndex")]
    pub validator_index: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SsvMessageProcessingTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: Option<String>,
    #[serde(rename = "Documentation")]
    pub documentation: Option<String>,

    // Fields for single test format (MsgProcessingSpecTest_*)
    #[serde(rename = "Runner")]
    pub runner: Option<RunnerConfig>,
    #[serde(rename = "ValidatorDuty")]
    pub validator_duty: Option<ValidatorDuty>,
    #[serde(rename = "CommitteeDuty")]
    pub committee_duty: Option<CommitteeDuty>,
    #[serde(rename = "Messages")]
    pub messages: Option<Vec<FlexibleSignedSSVMessage>>,
    #[serde(rename = "PostDutyRunnerStateRoot")]
    pub post_duty_runner_state_root: Option<String>,
    #[serde(rename = "OutputMessages")]
    pub output_messages: Option<Vec<OutputMessage>>,
    #[serde(rename = "BeaconBroadcastedRoots")]
    pub beacon_broadcasted_roots: Option<Vec<String>>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: Option<String>,
    #[serde(rename = "DecidedSlashable")]
    pub decided_slashable: Option<bool>,
    #[serde(rename = "DontStartDuty")]
    pub dont_start_duty: Option<bool>,

    // Field for multi test format (MultiMsgProcessingSpecTest_*)
    #[serde(rename = "Tests")]
    pub tests: Option<Vec<MessageProcessingSubTest>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MessageProcessingSubTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Runner")]
    pub runner: RunnerConfig,
    #[serde(rename = "ValidatorDuty")]
    pub validator_duty: Option<ValidatorDuty>,
    #[serde(rename = "CommitteeDuty")]
    pub committee_duty: Option<CommitteeDuty>,
    #[serde(rename = "Messages")]
    pub messages: Vec<FlexibleSignedSSVMessage>,
    #[serde(rename = "PostDutyRunnerStateRoot")]
    pub post_duty_runner_state_root: String,
    #[serde(rename = "OutputMessages")]
    pub output_messages: Vec<OutputMessage>,
    #[serde(rename = "BeaconBroadcastedRoots")]
    pub beacon_broadcasted_roots: Option<Vec<String>>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
    #[serde(rename = "DecidedSlashable")]
    pub decided_slashable: Option<bool>,
    #[serde(rename = "DontStartDuty")]
    pub dont_start_duty: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerConfig {
    #[serde(rename = "BaseRunner")]
    pub base_runner: BaseRunnerConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BaseRunnerConfig {
    #[serde(rename = "State")]
    pub state: Option<RunnerState>,
    #[serde(rename = "Share")]
    pub share: HashMap<String, JsonShare>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerState {
    #[serde(rename = "PreConsensusContainer")]
    pub pre_consensus_container: Option<PartialSigContainer>,
    #[serde(rename = "PostConsensusContainer")]
    pub post_consensus_container: Option<PartialSigContainer>,
    #[serde(rename = "RunningInstance")]
    pub running_instance: Option<Value>,
    #[serde(rename = "DecidedValue")]
    #[serde(deserialize_with = "deserialize_optional_base64_or_vec")]
    pub decided_value: Option<Vec<u8>>,
    #[serde(rename = "Finished")]
    pub finished: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PartialSigContainer {
    #[serde(rename = "Signatures")]
    #[serde(deserialize_with = "deserialize_signature_map")]
    pub signatures: HashMap<String, HashMap<String, HashMap<String, Vec<u8>>>>,
    #[serde(rename = "Quorum")]
    pub quorum: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ValidatorDuty {
    #[serde(rename = "Type")]
    pub duty_type: u64,
    #[serde(rename = "PubKey")]
    pub pub_key: String,
    #[serde(rename = "Slot")]
    pub slot: String,
    #[serde(rename = "ValidatorIndex")]
    pub validator_index: String,
    #[serde(rename = "CommitteeIndex")]
    pub committee_index: Option<u64>,
    #[serde(rename = "CommitteeLength")]
    pub committee_length: Option<u64>,
    #[serde(rename = "CommitteeValidatorIndex")]
    pub committee_validator_index: Option<u64>,
    #[serde(rename = "CommitteesAtSlot")]
    pub committees_at_slot: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommitteeDuty {
    #[serde(rename = "Slot")]
    pub slot: String,
    #[serde(rename = "ValidatorDuties")]
    pub validator_duties: Vec<ValidatorDuty>,
}

impl SpecTest for SsvMessageProcessingTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) {
        // No-op for parsing validation
    }

    fn run(&self) -> bool {
        // todo!()
        true
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Ssv(SsvSpecTestType::MessageProcessing)
    }
}

// Newtype wrapper for MultiMessageProcessing that uses the same data but different test type
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SsvMultiMessageProcessingTest(pub SsvMessageProcessingTest);

impl SpecTest for SsvMultiMessageProcessingTest {
    fn name(&self) -> &str {
        &self.0.name
    }

    fn setup(&mut self) {
        // No-op for parsing validation
    }

    fn run(&self) -> bool {
        // todo!()
        true
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Ssv(SsvSpecTestType::MultiMessageProcessing)
    }
}
