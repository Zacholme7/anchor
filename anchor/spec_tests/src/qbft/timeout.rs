use super::adapters::qbft::QbftAdapter;
use super::adapters::spec_types::{
    AcceptedProposal, ExpectedTimerState, MessageContainer, SpecTestCommitteeMember,
    TestSignedSSVMessage,
};
use crate::utils::deserializers::{
    deserialize_base64, deserialize_base64_option, deserialize_hex_hash256,
};
use crate::utils::test_keys::TestKeySet;
use crate::{QbftSpecTestType, SpecTest, SpecTestType};
use serde::Deserialize;
use ssv_types::message::SignedSSVMessage;
use tree_hash::TreeHash;
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
    #[serde(serialize_with = "serialize_base64")]
    pub id: Vec<u8>,
    #[serde(rename = "Round")]
    pub round: u64,
    #[serde(rename = "Height")]
    pub height: u64,
    #[serde(rename = "LastPreparedRound")]
    pub last_prepared_round: u64,
    #[serde(rename = "LastPreparedValue")]
    #[serde(deserialize_with = "deserialize_base64_option")]
    #[serde(serialize_with = "serialize_base64_option")]
    pub last_prepared_value: Option<Vec<u8>>,
    #[serde(rename = "ProposalAcceptedForCurrentRound")]
    pub proposal_accepted_for_current_round: Option<AcceptedProposal>,
    #[serde(rename = "Decided")]
    pub decided: bool,
    #[serde(rename = "DecidedValue")]
    #[serde(deserialize_with = "deserialize_base64_option")]
    #[serde(serialize_with = "serialize_base64_option")]
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

impl SpecTest for TimeoutTest {
    fn run(&self) -> bool {
        // Get test keys
        let test_keys = TestKeySet::four_share_set();

        // Create adapter from Pre state
        let mut adapter = QbftAdapter::from_timeout_pre(&self.pre, &test_keys);

        // Record initial round
        let initial_round = adapter.get_round();

        // Trigger timeout and handle potential error
        match adapter.trigger_timeout() {
            Ok(()) => {
                // Timeout succeeded - check if we expected an error
                if !self.expected_error.is_empty() {
                    return false;
                }
            }
            Err(err) => {
                // Timeout returned an error - check if it matches expected
                if self.expected_error.is_empty() || err != self.expected_error {
                    return false;
                }

                // For error cases , verify state unchanged
                if adapter.get_round() != initial_round {
                    return false;
                }

                // Verify no messages were sent
                if !adapter.get_captured_messages().is_empty() {
                    return false;
                }

                // Error case handled correctly
                return true;
            }
        }

        // Make sure round was incremented
        let new_round = adapter.get_round();
        if new_round != initial_round + 1 {
            return false;
        }

        // Check timer state if provided
        if let Some(expected_timer) = &self.expected_timer_state {
            // Validate the round if specified
            if let Some(expected_round) = expected_timer.round {
                if new_round != expected_round {
                    return false;
                }
            }

            // Validate the timeout count
            let timeout_count = adapter.get_timeout_count();
            if timeout_count != expected_timer.timeouts {
                return false;
            }
        }

        // Check output messages
        let captured = adapter.get_captured_messages();

        // Verify signatures on all captured messages
        if test_keys.verify_signed_messages(&captured).is_err() {
            return false;
        }

        if let Some(expected_msgs) = &self.output_messages {
            if captured.len() != expected_msgs.len() {
                return false;
            }

            // Validate each message matches expected by comparing roots
            for (captured_msg, expected_msg) in captured.iter().zip(expected_msgs.iter()) {
                let expected_msg: SignedSSVMessage =
                    expected_msg.clone().try_into().expect("Valid Message");
                if captured_msg.tree_hash_root() != expected_msg.tree_hash_root() {
                    return false;
                }
            }
        }

        // TODO: Post-state root validation

        true
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::Timeout)
    }
}
