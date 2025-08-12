use crate::types::TestSignedSSVMessage;
use crate::utils::deserializers::{deserialize_base64, deserialize_hex};
use serde::Deserialize;
use std::collections::HashMap;

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
