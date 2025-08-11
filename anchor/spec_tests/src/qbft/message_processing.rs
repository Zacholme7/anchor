use super::adapters::spec_types::SpecTestCommitteeMember;
use crate::types::TestSignedSSVMessage;
use crate::utils::deserializers::{deserialize_base64, deserialize_base64_option, deserialize_hex};
use crate::{QbftSpecTestType, SpecTest, SpecTestType};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct MessageProcessingState {
    #[serde(rename = "CommitteeMember")]
    pub committee_member: SpecTestCommitteeMember,
    #[serde(rename = "ID")]
    #[serde(deserialize_with = "deserialize_base64")]
    pub id: Vec<u8>,
    #[serde(rename = "Round")]
    pub round: u64,
    #[serde(rename = "Height")]
    pub height: u64,
    #[serde(rename = "LastPreparedRound")]
    pub last_prepared_round: u64,
    #[serde(rename = "LastPreparedValue")]
    #[serde(deserialize_with = "deserialize_base64_option")]
    pub last_prepared_value: Option<Vec<u8>>,
    #[serde(rename = "ProposalAcceptedForCurrentRound")]
    pub proposal_accepted_for_current_round: Option<AcceptedProposal>,
    #[serde(rename = "Decided")]
    pub decided: bool,
    #[serde(rename = "DecidedValue")]
    #[serde(deserialize_with = "deserialize_base64_option")]
    pub decided_value: Option<Vec<u8>>,
    #[serde(rename = "ProposeContainer")]
    pub propose_container: MessageContainer,
    #[serde(rename = "PrepareContainer")]
    pub prepare_container: MessageContainer,
    #[serde(rename = "CommitContainer")]
    pub commit_container: MessageContainer,
    #[serde(rename = "RoundChangeContainer")]
    pub round_change_container: MessageContainer,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AcceptedProposal {
    #[serde(rename = "SignedMessage")]
    pub signed_message: TestSignedSSVMessage,
    #[serde(rename = "QBFTMessage")]
    pub qbft_message: QbftMessageData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QbftMessageData {
    #[serde(rename = "MsgType")]
    pub msg_type: u64,
    #[serde(rename = "Height")]
    pub height: u64,
    #[serde(rename = "Round")]
    pub round: u64,
    #[serde(rename = "Identifier")]
    #[serde(deserialize_with = "deserialize_base64")]
    pub identifier: Vec<u8>,
    #[serde(rename = "Root")]
    #[serde(deserialize_with = "deserialize_hex")]
    pub root: Vec<u8>,
    #[serde(rename = "DataRound")]
    pub data_round: u64,
    #[serde(rename = "RoundChangeJustification")]
    pub round_change_justification: Vec<serde_json::Value>,
    #[serde(rename = "PrepareJustification")]
    pub prepare_justification: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageContainer {
    #[serde(rename = "Msgs")]
    pub msgs: std::collections::HashMap<String, TestSignedSSVMessage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageProcessingPre {
    #[serde(rename = "forceStop")]
    #[serde(default)]
    pub force_stop: bool,
    #[serde(rename = "State")]
    pub state: MessageProcessingState,
    #[serde(rename = "StartValue")]
    #[serde(deserialize_with = "deserialize_base64")]
    pub start_value: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedTimerState {
    #[serde(rename = "Timeouts")]
    pub timeouts: u64,
    #[serde(rename = "Round")]
    pub round: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageProcessingTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(rename = "Pre")]
    pub pre: MessageProcessingPre,
    #[serde(rename = "InputMessages")]
    pub input_messages: Vec<TestSignedSSVMessage>,
    #[serde(rename = "OutputMessages")]
    pub output_messages: Option<Vec<TestSignedSSVMessage>>,
    #[serde(rename = "PostRoot")]
    #[serde(deserialize_with = "deserialize_base64_option")]
    pub post_root: Option<Vec<u8>>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
    #[serde(rename = "ExpectedTimerState")]
    pub expected_timer_state: Option<ExpectedTimerState>,
}

impl SpecTest for MessageProcessingTest {
    fn setup(&mut self) {}

    fn run(&self) -> bool {
        true
    }
    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::MsgProcessing)
    }
}
