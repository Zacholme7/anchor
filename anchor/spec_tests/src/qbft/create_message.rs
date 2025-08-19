use super::adapters::qbft::*;

// Hardcoded SSZ-encoded BeaconVote for create message tests (matches Go's TestingQBFTFullData)
const TESTING_BEACON_VOTE_SSZ: &[u8] = &[
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3,
];
use super::adapters::spec_types::{SpecTestCommitteeMember, TestSignedSSVMessage};
use crate::utils::deserializers::{
    deserialize_base64_option, deserialize_create_type, deserialize_hex, deserialize_hex_hash256,
};
use crate::{QbftSpecTestType, SpecTest, SpecTestType};
use serde::Deserialize;
use ssv_types::consensus::{QbftMessage, QbftMessageType};
use ssv_types::message::SignedSSVMessage;
use ssv_types::msgid::MessageId;
use ssz::Decode;
use std::cell::RefCell;
use tree_hash::TreeHash;
use types::Hash256;

#[derive(Deserialize)]
pub struct CreateMessageTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(rename = "CreateType", deserialize_with = "deserialize_create_type")]
    pub msg_type: QbftMessageType,
    #[serde(rename = "Value")]
    #[serde(deserialize_with = "deserialize_hex")]
    pub root: Vec<u8>, // JSON has this as hex string
    #[serde(rename = "Round")]
    pub round: Option<u64>,
    #[serde(rename = "StateValue")]
    #[serde(deserialize_with = "deserialize_base64_option")]
    pub value: Option<Vec<u8>>,
    #[serde(rename = "RoundChangeJustifications")]
    pub round_change_justifications: Option<Vec<TestSignedSSVMessage>>,
    #[serde(rename = "PrepareJustifications")]
    pub prepare_justifications: Option<Vec<TestSignedSSVMessage>>,
    #[serde(rename = "ExpectedRoot")]
    #[serde(deserialize_with = "deserialize_hex_hash256")]
    pub expected_root: Hash256,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
    #[serde(rename = "OperatorID")]
    pub operator_id: Option<u64>,
    #[serde(rename = "CommitteeMember")]
    pub committee_member: SpecTestCommitteeMember,
    #[serde(rename = "Identifier")]
    #[serde(deserialize_with = "deserialize_base64_option")]
    pub identifier: Option<Vec<u8>>,
    #[serde(skip)]
    qbft_adapter: RefCell<Option<QbftAdapter>>,
}

impl SpecTest for CreateMessageTest {
    fn setup(&mut self) {
        use crate::utils::test_keys::TestKeySet;

        // Get the test keys
        let test_keys = TestKeySet::four_share_set();

        // Always use the identifier from JSON (it's always [1,2,3,4,0,0,...])
        let identifier = self.identifier.as_ref().and_then(|bytes| {
            if bytes.len() == 56 {
                Some(MessageId::from(
                    <[u8; 56]>::try_from(bytes.as_slice()).ok()?,
                ))
            } else {
                None
            }
        });

        // Go always uses FirstHeight (0) for the instance, not the round
        let height = Some(qbft::InstanceHeight::from(0));

        // Committee is always provided in JSON now
        let committee = Some(
            self.committee_member
                .committee
                .iter()
                .map(|op| ssv_types::OperatorId::from(op.operator_id))
                .collect(),
        );

        // Handle operator ID:
        // - If OperatorID field is set at root level, use it
        // - If CommitteeMember.OperatorID is 0, use default operator 1
        // - Otherwise use CommitteeMember.OperatorID
        let operator_id = if let Some(op_id) = self.operator_id {
            ssv_types::OperatorId::from(op_id)
        } else if self.committee_member.operator_id == ssv_types::OperatorId::from(0) {
            // Operator ID 0 is a placeholder meaning "use default test operator"
            ssv_types::OperatorId::from(1)
        } else {
            self.committee_member.operator_id
        };

        // Get the RSA key for this operator
        let operator_rsa_key = test_keys.operator_keys.get(&operator_id).cloned();

        let starting_state = QbftStartingState {
            height: height.unwrap_or(qbft::InstanceHeight::from(0)),
            identifier: identifier.unwrap_or_else(|| MessageId::from([0u8; 56])),
            committee,
            operator_id,
            round: self
                .round
                .map(|r| ssv_types::Round::from(r))
                .unwrap_or(ssv_types::Round::from(0)),
            start_value: TESTING_BEACON_VOTE_SSZ.to_vec(),
        };
        let mut adapter = QbftAdapter::new_with_state(starting_state);

        // For RoundChange messages with prepare justifications AND StateValue, set up the state
        // Go sets LastPreparedValue = test.StateValue, so if StateValue is null, LastPreparedValue is null
        if self.msg_type == ssv_types::consensus::QbftMessageType::RoundChange {
            if let Some(prep_justifications) = &self.prepare_justifications {
                if !prep_justifications.is_empty() && self.value.is_some() {
                    // Only setup if we have StateValue
                    // Convert TestSignedSSVMessage to SignedSSVMessage
                    let prepare_msgs: Vec<SignedSSVMessage> = prep_justifications
                        .iter()
                        .filter_map(|msg| msg.clone().try_into().ok())
                        .collect();

                    // Setup prepare justifications with StateValue
                    let state_value = self.value.as_ref().map(|v| v.as_slice());
                    // If setup fails, it will be caught when the test runs
                    let _ = adapter.setup_prepare_justifications(&prepare_msgs, state_value);
                }
                // If StateValue is null, don't setup (LastPreparedValue remains null)
            }
        }

        self.qbft_adapter = RefCell::new(Some(adapter));
    }

    fn run(&self) -> bool {
        println!("Running create message test: {}", self.name);
        if let Some(mut adapter) = self.qbft_adapter.borrow_mut().take() {
            // Convert TestSignedSSVMessage to SignedSSVMessage
            let rc_justifications = self.round_change_justifications.as_ref().map(|msgs| {
                msgs.iter()
                    .filter_map(|msg| msg.clone().try_into().ok())
                    .collect()
            });
            let prep_justifications: Option<Vec<SignedSSVMessage>> =
                self.prepare_justifications.as_ref().map(|msgs| {
                    msgs.iter()
                        .filter_map(|msg| msg.clone().try_into().ok())
                        .collect()
                });

            // Use the Value field as raw data (matches Go)
            // The adapter will hash it to get the root
            let data_bytes = &self.root;

            // For RoundChange, Go always passes FirstRound (1) to CreateRoundChange
            // For other message types, use test.Round
            let round_param = if self.msg_type == ssv_types::consensus::QbftMessageType::RoundChange
            {
                Some(1) // FirstRound constant in Go
            } else {
                self.round
            };

            // For RoundChange: only pass prepare justifications if we have StateValue AND quorum
            // Go's getRoundChangeJustification returns nil if no LastPreparedValue (which comes from StateValue)
            let prep_justifications_to_pass =
                if self.msg_type == ssv_types::consensus::QbftMessageType::RoundChange {
                    // First check if we have StateValue
                    if self.value.is_some() {
                        // Now check for quorum
                        if let Some(ref preps) = prep_justifications {
                            if has_quorum(preps) {
                                // Have StateValue AND quorum, include justifications
                                prep_justifications
                            } else {
                                // Have StateValue but no quorum, don't include justifications
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        // No StateValue, never include justifications
                        None
                    }
                } else {
                    // For other message types, pass as-is
                    prep_justifications
                };

            let signed_ssv_message = adapter
                .create_message(
                    self.msg_type,
                    data_bytes,
                    &rc_justifications,
                    &prep_justifications_to_pass,
                    round_param,
                )
                .expect("create_message should not fail for valid test cases");

            // compare the roots
            let root = signed_ssv_message.tree_hash_root();
            if root != self.expected_root {
                println!("❌ Root mismatch!");
                println!("  Expected: {:?}", self.expected_root);
                println!("  Actual:   {:?}", root);
                return false;
            }

            // Validate the SignedSSVMessage
            if signed_ssv_message.validate().is_err() {
                return false;
            }

            // deser the qbft message and also call validate on that
            let Ok(_qbft_message) =
                QbftMessage::from_ssz_bytes(signed_ssv_message.ssv_message().data())
            else {
                return false;
            };

            // TODO: QbftMessage validation is not implemented in our types yet
            //if qbft_message.validate().is_err() {
            //    return false;
            //}

            // State comparison is not needed for create message tests
            return true;
        }
        false
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::CreateMessage)
    }
}

/// Check if a set of messages has quorum (3 out of 4 for test committee)
fn has_quorum(messages: &[SignedSSVMessage]) -> bool {
    let mut unique_signers = std::collections::HashSet::new();
    for msg in messages {
        for op_id in msg.operator_ids() {
            unique_signers.insert(*op_id);
        }
    }
    // For 4-node committee, quorum is 3
    unique_signers.len() >= 3
}
