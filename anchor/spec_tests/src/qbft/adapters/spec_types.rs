use crate::types::TestSignedSSVMessage;
use crate::utils::deserializers::{deserialize_base64, deserialize_hex};
use serde::Deserialize;
use ssv_types::OperatorId;
use std::collections::HashMap;

/// Committee member as defined by the spec. Used for parsing
/// and then covnerted into our internal types
#[derive(Debug, Clone, Deserialize)]
pub struct SpecTestCommitteeMember {
    #[serde(rename = "OperatorID")]
    pub operator_id: OperatorId,
    #[serde(rename = "CommitteeID")]
    #[serde(deserialize_with = "deserialize_hex")]
    pub committee_id: Vec<u8>,
    #[serde(rename = "SSVOperatorPubKey")]
    pub ssv_operator_pub_key: Option<String>,
    #[serde(rename = "FaultyNodes")]
    pub faulty_nodes: u64,
    #[serde(rename = "Committee")]
    pub committee: Option<Vec<SpecTestOperator>>,
    #[serde(rename = "DomainType")]
    #[serde(deserialize_with = "deserialize_hex")]
    pub domain_type: Vec<u8>,
}

/// Operator from the spec test
#[derive(Debug, Clone, Deserialize)]
pub struct SpecTestOperator {
    #[serde(rename = "OperatorID")]
    pub operator_id: u64,
    #[serde(rename = "SSVOperatorPubKey")]
    pub ssv_operator_pub_key: String,
}

/// Timer state expected after test execution
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedTimerState {
    #[serde(rename = "Timeouts")]
    pub timeouts: u64,
    #[serde(rename = "Round")]
    pub round: Option<u64>,
}

/// Container for QBFT messages indexed by a key
#[derive(Debug, Clone, Deserialize)]
pub struct MessageContainer {
    #[serde(rename = "Msgs")]
    pub msgs: HashMap<String, TestSignedSSVMessage>,
}

/// Accepted proposal for the current round
#[derive(Debug, Clone, Deserialize)]
pub struct AcceptedProposal {
    #[serde(rename = "SignedMessage")]
    pub signed_message: TestSignedSSVMessage,
    #[serde(rename = "QBFTMessage")]
    pub qbft_message: QbftMessageData,
}

/// QBFT message data structure
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
