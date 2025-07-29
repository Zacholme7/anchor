use super::adapter::{QbftTestAdapter, TestContext, TestType};
use crate::{QbftSpecTestType, SpecTest, SpecTestType};
use serde::Deserialize;
use ssv_types::{Round, message::SignedSSVMessage};
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
    pub post_root: Hash256,
    #[serde(rename = "OutputMessages")]
    pub output_messages: Option<Vec<SignedSSVMessage>>,
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
    pub start_value: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QbftInstanceState {
    #[serde(rename = "CommitteeMember")]
    pub committee_member: super::adapter::SpecTestCommitteeMember,
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Round")]
    pub round: u64,
    #[serde(rename = "Height")]
    pub height: u64,
    #[serde(rename = "LastPreparedRound")]
    pub last_prepared_round: u64,
    #[serde(rename = "LastPreparedValue")]
    pub last_prepared_value: Option<String>,
    #[serde(rename = "ProposalAcceptedForCurrentRound")]
    pub proposal_accepted_for_current_round: Option<AcceptedProposal>,
    #[serde(rename = "Decided")]
    pub decided: bool,
    #[serde(rename = "DecidedValue")]
    pub decided_value: Option<String>,
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
    pub signed_message: SignedSSVMessage,
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

    fn run(&self) -> bool {}

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::Timeout)
    }
}
