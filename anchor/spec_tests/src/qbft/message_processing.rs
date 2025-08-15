use super::adapters::qbft::QbftAdapter;
use super::adapters::spec_types::{
    AcceptedProposal, ExpectedTimerState, MessageContainer, SpecTestCommitteeMember,
    TestSignedSSVMessage,
};
use crate::utils::deserializers::{deserialize_base64, deserialize_base64_option};
use crate::utils::test_keys::TestKeySet;
use crate::{QbftSpecTestType, SpecTest, SpecTestType};

use serde::Deserialize;
use ssv_types::consensus::QbftMessage;
use ssz::Decode;

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
    fn run(&self) -> bool {
        // Get test keys
        let test_keys = TestKeySet::four_share_set();

        // Create adapter from Pre state
        let mut adapter = match QbftAdapter::from_message_processing_pre(&self.pre, &test_keys) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("FAILED {}: Failed to create adapter: {}", self.name, e);
                return false;
            }
        };

        // Process each input message
        let mut last_error = None;
        for (_i, msg) in self.input_messages.iter().enumerate() {
            if let Err(e) = adapter.process_message(msg) {
                last_error = Some(e);
                // Don't break - continue processing all messages
                // Go tests continue processing even after errors
            }
        }

        // Check error expectations
        if !self.expected_error.is_empty() {
            match last_error {
                Some(e) if e == self.expected_error => {
                    // Expected error matched
                }
                Some(e) => {
                    eprintln!(
                        "FAILED {}: Expected error '{}', got '{}'",
                        self.name, self.expected_error, e
                    );
                    return false;
                }
                None => {
                    eprintln!(
                        "FAILED {}: Expected error '{}', got none",
                        self.name, self.expected_error
                    );
                    return false;
                }
            }
        } else if let Some(e) = last_error {
            eprintln!("FAILED {}: Unexpected error: {}", self.name, e);
            return false;
        }

        // Check output messages
        if let Some(expected_msgs) = &self.output_messages {
            let captured = adapter.get_captured_messages();
            if captured.len() != expected_msgs.len() {
                eprintln!("DEBUG: Captured messages for '{}':", self.name);
                for (i, msg) in captured.iter().enumerate() {
                    if let Ok(qbft_msg) = QbftMessage::from_ssz_bytes(msg.ssv_message().data()) {
                        eprintln!(
                            "  {}: {:?} round {}",
                            i, qbft_msg.qbft_message_type, qbft_msg.round
                        );
                    }
                }
                eprintln!("Expected messages:");
                for (i, msg) in expected_msgs.iter().enumerate() {
                    if let Some(ssv_msg) = &msg.ssv_message {
                        if let Ok(qbft_msg) = QbftMessage::from_ssz_bytes(ssv_msg.data()) {
                            eprintln!(
                                "  {}: {:?} round {}",
                                i, qbft_msg.qbft_message_type, qbft_msg.round
                            );
                        }
                    }
                }
                eprintln!(
                    "FAILED {}: Expected {} output messages, got {}",
                    self.name,
                    expected_msgs.len(),
                    captured.len()
                );
                return false;
            }

            // For now, just check counts - full message validation can be added later
            // (similar to timeout tests)
        }

        // TODO: Check timer state if provided
        // TODO: Check post-state root (same JSON issues as timeout tests)

        true
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::MsgProcessing)
    }
}
