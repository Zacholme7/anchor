use super::adapters::spec_types::SpecTestCommitteeMember;
use crate::types::TestSignedSSVMessage;
use crate::utils::deserializers::{
    deserialize_base64, deserialize_base64_option, deserialize_hex_hash256,
};
use crate::{QbftSpecTestType, SpecTest, SpecTestType};
use serde::Deserialize;
use types::Hash256;

#[derive(Debug, Clone, Deserialize)]
pub struct TimeoutTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(rename = "Pre")]
    pub pre: TimeoutTestPre,
    #[serde(rename = "PostRoot")]
    #[serde(deserialize_with = "deserialize_hex_hash256")]
    pub post_root: Hash256,
    #[serde(rename = "OutputMessages")]
    pub output_messages: Option<Vec<TestSignedSSVMessage>>,
    #[serde(rename = "ExpectedTimerState")]
    pub expected_timer_state: Option<ExpectedTimerState>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimeoutTestPre {
    #[serde(rename = "State")]
    pub state: QbftInstanceState,
    #[serde(rename = "StartValue")]
    #[serde(deserialize_with = "deserialize_base64_option")]
    pub start_value: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QbftInstanceState {
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
    pub qbft_message: serde_json::Value, // Raw JSON for now
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageContainer {
    #[serde(rename = "Msgs")]
    pub msgs: serde_json::Value, // Raw JSON for now
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedTimerState {
    #[serde(rename = "Timeouts")]
    pub timeouts: u64,
    #[serde(rename = "Round")]
    pub round: u64,
}

impl SpecTest for TimeoutTest {
    fn setup(&mut self) {
        // No setup needed for timeout tests
    }

    fn run(&self) -> bool {
        // TODO: Implement timeout test logic
        false
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::Timeout)
    }
}
