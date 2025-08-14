use crate::utils::deserializers::{deserialize_base64, deserialize_hex};
use crate::utils::error_mapping::map_signed_message_error;
use crate::utils::misc::create_beacon_vote_from_bytes;
use base64::prelude::*;
use qbft::WrappedQbftMessage;
use serde::Deserialize;
use ssv_types::{
    OperatorId,
    consensus::{QbftMessage, QbftMessageType},
    message::{SSVMessage, SignedSSVMessage},
};
use ssz::{Decode, Encode};
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

// Intermediate test-specific SignedSSVMessage that can handle null SSVMessage
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestSignedSSVMessage {
    #[serde(rename = "Signatures")]
    pub signatures: Vec<String>,
    #[serde(rename = "OperatorIDs")]
    pub operator_ids: Option<Vec<OperatorId>>,
    #[serde(rename = "SSVMessage")]
    pub ssv_message: Option<SSVMessage>,
    #[serde(rename = "FullData")]
    pub full_data: Option<String>,
}

impl TryFrom<TestSignedSSVMessage> for SignedSSVMessage {
    type Error = String;

    fn try_from(test_msg: TestSignedSSVMessage) -> Result<Self, Self::Error> {
        // Convert signatures from base64 strings to [u8; 256] arrays
        let mut signatures = Vec::new();
        for sig_str in &test_msg.signatures {
            let sig_bytes = BASE64_STANDARD
                .decode(sig_str.as_bytes())
                .map_err(|e| format!("Failed to decode signature: {}", e))?;

            if sig_bytes.len() != 256 {
                return Err(format!(
                    "Invalid signature length: expected 256, got {}",
                    sig_bytes.len()
                ));
            }

            let mut sig_array = [0u8; 256];
            sig_array.copy_from_slice(&sig_bytes);
            signatures.push(sig_array);
        }

        // Get SSV message or error
        let ssv_message = test_msg
            .ssv_message
            .clone()
            .ok_or_else(|| "SSVMessage is None".to_string())?;

        // Decode full_data from base64 string to bytes
        let full_data_bytes = match &test_msg.full_data {
            Some(base64_str) => BASE64_STANDARD
                .decode(base64_str.as_bytes())
                .map_err(|e| format!("failed to decode base64 full_data: {}", e))?,
            None => Vec::new(),
        };

        // Create our SignedSSVMessage
        SignedSSVMessage::new(
            signatures,
            test_msg.operator_ids.clone().unwrap_or_default(),
            ssv_message,
            full_data_bytes,
        )
        .map_err(|e| format!("Failed to create SignedSSVMessage: {:?}", e))
    }
}

impl TestSignedSSVMessage {
    /// Convert to WrappedQbftMessage for processing by core QBFT
    pub fn to_wrapped_qbft_message(&self) -> Result<WrappedQbftMessage, String> {
        // Get the SSVMessage from the test message
        let ssv_message = self
            .ssv_message
            .as_ref()
            .ok_or_else(|| "TestSignedSSVMessage has null SSVMessage".to_string())?;

        // Decode QBFT message first to check type
        let qbft_message = QbftMessage::from_ssz_bytes(ssv_message.data())
            .map_err(|e| format!("Failed to decode QBFT message: {:?}", e))?;

        // Convert signatures - test messages have base64 encoded signatures
        let signatures: Vec<[u8; 256]> = self
            .signatures
            .iter()
            .map(|sig_str| {
                let sig_bytes = BASE64_STANDARD
                    .decode(sig_str)
                    .map_err(|e| format!("Failed to decode signature: {:?}", e))?;
                if sig_bytes.len() != 256 {
                    return Err(format!("Invalid signature length: {}", sig_bytes.len()));
                }
                let mut sig = [0u8; 256];
                sig.copy_from_slice(&sig_bytes);
                Ok(sig)
            })
            .collect::<Result<Vec<_>, String>>()?;

        // Convert operator IDs
        let operator_ids: Vec<OperatorId> = self
            .operator_ids
            .as_ref()
            .ok_or_else(|| "TestSignedSSVMessage has null OperatorIDs".to_string())?
            .clone();

        // Handle full_data conversion based on message type
        let full_data = if let Some(data_str) = &self.full_data {
            let raw_data = BASE64_STANDARD
                .decode(data_str)
                .map_err(|e| format!("Failed to decode full_data: {:?}", e))?;

            if !raw_data.is_empty() && qbft_message.qbft_message_type == QbftMessageType::Proposal {
                // For proposals, keep the full_data for validation
                // The core will validate H(data) == root
                raw_data
            } else if !raw_data.is_empty()
                && qbft_message.qbft_message_type == QbftMessageType::RoundChange
            {
                // For RoundChange with prepared value, encode as BeaconVote
                if qbft_message.data_round > 0 {
                    let beacon_vote = create_beacon_vote_from_bytes(&raw_data);
                    beacon_vote.as_ssz_bytes()
                } else {
                    raw_data
                }
            } else {
                // For other types or empty data, keep as is
                raw_data
            }
        } else {
            vec![]
        };

        // Create SignedSSVMessage - this may fail for invalid messages (e.g., duplicate signers)
        // We need to propagate this error so it can be matched against expected errors
        // Note: We create the message first to let validation catch issues like duplicate signers
        let signed_ssv_message =
            match SignedSSVMessage::new(signatures, operator_ids.clone(), ssv_message.clone(), full_data) {
                Ok(msg) => msg,
                Err(e) => {
                    // Convert the error to match Go's format using centralized mapping
                    let error_str = map_signed_message_error(&e);
                    return Err(error_str);
                }
            };
        
        // Check for multi-signers after successful creation
        // Multi-signers are only allowed for commit messages
        // Note: This check is for valid messages with multiple unique signers
        if signed_ssv_message.operator_ids().len() > 1 && qbft_message.qbft_message_type != QbftMessageType::Commit {
            return Err("invalid signed message: msg allows 1 signer".to_string());
        }

        // Create WrappedQbftMessage (we already decoded qbft_message above)
        Ok(WrappedQbftMessage {
            signed_message: signed_ssv_message,
            qbft_message,
        })
    }
}
