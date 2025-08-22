use qbft::InstanceHeight;
use serde::Deserialize;
use ssv_types::{
    IndexSet, OperatorId, Round,
    consensus::{QbftMessage, QbftMessageType},
    message::SignedSSVMessage,
    msgid::MessageId,
};
use ssz::Decode;
use tree_hash::TreeHash;
use types::Hash256;

use super::adapters::{
    qbft::{QbftAdapter, QbftStartingState},
    spec_types::{MessageContainer, SpecTestCommitteeMember, TestSignedSSVMessage},
};
use crate::{
    QbftSpecTestType, SpecTest, SpecTestType,
    utils::deserializers::{
        deserialize_base64, deserialize_base64_option, deserialize_create_type,
        deserialize_hex_hash256,
    },
};

#[derive(Deserialize)]
pub struct CreateMessageTest {
    #[serde(rename = "Name")]
    pub name: String,

    #[serde(rename = "Type")]
    pub test_type: String,

    #[serde(rename = "Documentation")]
    pub documentation: String,

    #[serde(rename = "Value")]
    #[serde(deserialize_with = "deserialize_hex_hash256")]
    pub root: Hash256,

    #[serde(rename = "StateValue")]
    #[serde(deserialize_with = "deserialize_base64_option")]
    pub value: Option<Vec<u8>>,

    #[serde(rename = "Round")]
    pub round: Option<u64>,

    #[serde(rename = "RoundChangeJustifications")]
    pub round_change_justifications: Option<Vec<TestSignedSSVMessage>>,

    #[serde(rename = "PrepareJustifications")]
    pub prepare_justifications: Option<Vec<TestSignedSSVMessage>>,

    #[serde(rename = "CreateType", deserialize_with = "deserialize_create_type")]
    pub msg_type: QbftMessageType,

    #[serde(rename = "ExpectedRoot")]
    #[serde(deserialize_with = "deserialize_hex_hash256")]
    pub expected_root: Hash256,

    #[serde(rename = "ExpectedError")]
    pub expected_error: String,

    #[serde(rename = "Identifier")]
    #[serde(deserialize_with = "deserialize_base64")]
    pub identifier: Vec<u8>,

    #[serde(rename = "CommitteeMember")]
    pub committee_member: SpecTestCommitteeMember,

    #[serde(rename = "OperatorID")]
    pub operator_id: Option<u64>,

    #[serde(skip)]
    qbft_state: Option<QbftStartingState>,
}

impl SpecTest for CreateMessageTest {
    fn setup(&mut self) {
        let committee = self.committee_member.committee.as_ref().map(|ops| {
            ops.iter()
                .map(|op| OperatorId::from(op.operator_id))
                .collect::<IndexSet<_>>()
        });

        // They all use operator 1 as the sighner
        let operator_id = OperatorId::from(1);
        println!("Test: {}", self.name);
        println!("Value: {:?}", self.value);
        println!("Round: {:?}", self.round);
        println!("Message Type: {:?}", self.msg_type);
        println!("Root: {:?}", self.root);
        println!("Committee: {:?}", committee);
        println!(
            "PrepareJustifications: {:?}",
            self.prepare_justifications.is_some()
        );
        println!(
            "RoundChangeJustifications: {:?}",
            self.round_change_justifications.is_some()
        );

        let starting_state = QbftStartingState {
            height: InstanceHeight::from(0),
            identifier: MessageId::from(<[u8; 56]>::try_from(self.identifier.as_slice()).unwrap()),
            committee,
            operator_id,
            round: self.round.map(|r| Round::from(r)).unwrap_or(Round::from(1)),
            start_value: self.value.clone().unwrap_or_default(),
            proposal_accepted: None,
            propose_container: MessageContainer::default(),
            prepare_container: MessageContainer::default(),
            commit_container: MessageContainer::default(),
            round_change_container: MessageContainer::default(),
            round_change_justifications: self.round_change_justifications.clone(),
            prepare_justifications: self.prepare_justifications.clone(),
            force_stop: false,
        };

        // Store the state for use in run()
        self.qbft_state = Some(starting_state.clone());
    }

    fn run(&self) -> bool {
        // Use the state constructed in setup()
        let state = self
            .qbft_state
            .as_ref()
            .expect("QbftStartingState should be initialized in setup()");

        println!("Creating adapter with state:");
        println!(
            "  prepare_justifications: {:?}",
            state.prepare_justifications.as_ref().map(|v| v.len())
        );
        println!(
            "  round_change_justifications: {:?}",
            state.round_change_justifications.as_ref().map(|v| v.len())
        );
        let mut adapter = QbftAdapter::new_with_state(state.clone());

        // Create the message
        let signed_ssv_message =
            adapter.create_message(self.msg_type, self.root, state.start_value.clone());

        // Deserialize the QBFT message immediately to inspect it
        let qbft_msg_from_created =
            QbftMessage::from_ssz_bytes(signed_ssv_message.ssv_message().data())
                .expect("Should deserialize created QBFT message");

        println!("\n=== Created Message Details ===");
        println!("SignedSSVMessage:");
        println!(
            "  msg_type: {:?}",
            signed_ssv_message.ssv_message().msg_type()
        );
        println!("  msg_id: {:?}", signed_ssv_message.ssv_message().msg_id());
        println!(
            "  data len: {}",
            signed_ssv_message.ssv_message().data().len()
        );
        println!("  full_data len: {}", signed_ssv_message.full_data().len());
        println!("  operator_ids: {:?}", signed_ssv_message.operator_ids());

        println!("\nQBFT Message (from created):");
        println!("  Type: {:?}", qbft_msg_from_created.qbft_message_type);
        println!("  Height: {:?}", qbft_msg_from_created.height);
        println!("  Round: {:?}", qbft_msg_from_created.round);
        println!("  Identifier: {:?}", qbft_msg_from_created.identifier);
        println!("  Root: {:?}", qbft_msg_from_created.root);
        println!("  DataRound: {:?}", qbft_msg_from_created.data_round);
        println!(
            "  RoundChangeJustification: {} messages",
            qbft_msg_from_created.round_change_justification.len()
        );
        println!(
            "  PrepareJustification: {} messages",
            qbft_msg_from_created.prepare_justification.len()
        );

        if qbft_msg_from_created.qbft_message_type == QbftMessageType::Proposal {
            // For proposals, print more details about justifications
            if !qbft_msg_from_created.round_change_justification.is_empty() {
                println!("  Round Change Justifications details:");
                for (i, rc_bytes) in qbft_msg_from_created
                    .round_change_justification
                    .iter()
                    .enumerate()
                {
                    if let Ok(rc_msg) = SignedSSVMessage::from_ssz_bytes(rc_bytes) {
                        if let Ok(rc_qbft) =
                            QbftMessage::from_ssz_bytes(rc_msg.ssv_message().data())
                        {
                            println!(
                                "    RC[{}]: round={}, type={:?}",
                                i, rc_qbft.round, rc_qbft.qbft_message_type
                            );
                        }
                    }
                }
            }
            if !qbft_msg_from_created.prepare_justification.is_empty() {
                println!("  Prepare Justifications details:");
                for (i, p_bytes) in qbft_msg_from_created
                    .prepare_justification
                    .iter()
                    .enumerate()
                {
                    if let Ok(p_msg) = SignedSSVMessage::from_ssz_bytes(p_bytes) {
                        if let Ok(p_qbft) = QbftMessage::from_ssz_bytes(p_msg.ssv_message().data())
                        {
                            println!(
                                "    Prepare[{}]: round={}, root={:?}",
                                i, p_qbft.round, p_qbft.root
                            );
                        }
                    }
                }
            }
        }
        println!("==============================\n");

        // Compare message root to expected root
        let root = signed_ssv_message.tree_hash_root();
        println!("Actual root:   {:?}", root);
        println!("Expected root: {:?}", self.expected_root);
        if root != self.expected_root {
            println!("roots dont match");
            return false;
        }

        // Validate the SignedSSVMessage
        if signed_ssv_message.validate().is_err() {
            println!("validation");
            return false;
        }

        // deser the qbft message and also call validate on that
        let Ok(qbft_message) = QbftMessage::from_ssz_bytes(signed_ssv_message.ssv_message().data())
        else {
            println!("could not deserialize qbft message");
            return false;
        };
        println!("QBFT Message deserialized:");
        println!("  Type: {:?}", qbft_message.qbft_message_type);
        println!("  Height: {:?}", qbft_message.height);
        println!("  Round: {:?}", qbft_message.round);
        println!("  Root: {:?}", qbft_message.root);

        // TODO: QbftMessage validation is not implemented in our types yet
        // if qbft_message.validate().is_err() {
        //    return false;
        //}

        // todo!() State comparison
        true
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::CreateMessage)
    }
}
