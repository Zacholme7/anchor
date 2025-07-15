mod common_types;
mod controller_test;
mod create_message;
mod qbft_message;
mod round_robin;
mod unified_test_adapter;
mod validation_adapter;

pub use common_types::{
    examples, CommitteeMember, Operator, QbftTestConfig, QbftTestScenario, ScenarioOutcome, TimeoutConfig,
};
pub use controller_test::ControllerTest;
pub use create_message::CreateMessageTest;
pub use qbft_message::QbftMessageTest;
pub use round_robin::RoundRobinTest;
use serde::{Deserialize, Deserializer};
use ssv_types::{Round, consensus::QbftMessageType};
use types::Hash256;

#[derive(Eq, PartialEq, Hash, Debug)]
pub(crate) enum QbftSpecTestType {
    QbftMessage,
    CreateMessage,
    RoundRobin,
    Controller,
}

// Contains specific identifier for the test file
impl std::fmt::Display for QbftSpecTestType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            QbftSpecTestType::QbftMessage => write!(f, "MsgSpecTest"),
            QbftSpecTestType::CreateMessage => write!(f, "CreateMsgSpecTest"),
            QbftSpecTestType::RoundRobin => write!(f, "RoundRobinSpecTest"),
            QbftSpecTestType::Controller => write!(f, "ControllerSpecTest"),
        }
    }
}

// Custom QBFT Specific serde deserializers
pub(crate) mod qbft_deserializers {
    use super::*;

    // Convert from string into QbftMessageType
    pub(crate) fn deserialize_qbft_message_type<'de, D>(
        deserializer: D,
    ) -> Result<QbftMessageType, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "createProposal" => Ok(QbftMessageType::Proposal),
            "CreatePrepare" => Ok(QbftMessageType::Prepare),
            "CreateCommit" => Ok(QbftMessageType::Commit),
            "CreateRoundChange" => Ok(QbftMessageType::RoundChange),
            _ => {
                eprintln!("DEBUG: Failed to parse QbftMessageType from: '{s}'");
                eprintln!(
                    "Valid options are: createProposal, CreatePrepare, CreateCommit, CreateRoundChange"
                );
                Err(serde::de::Error::custom(format!(
                    "Invalid message type: '{s}'. Valid options: createProposal, CreatePrepare, CreateCommit, CreateRoundChange"
                )))
            }
        }
    }

    // The Value field contains the actual data bytes that need to be hashed to get the root
    pub(crate) fn deserialize_value_into_root<'de, D>(deserializer: D) -> Result<Hash256, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Retrieve the bytes...
        let bytes = <Vec<u8>>::deserialize(deserializer).map_err(|e| {
            eprintln!("DEBUG: Failed to deserialize Value field as Vec<u8>: {e}");
            e
        })?;

        if bytes.len() != 32 {
            eprintln!(
                "DEBUG: Value field has {} bytes, expected 32 for Hash256",
                bytes.len()
            );
            eprintln!("DEBUG: Bytes: {bytes:?}");
            return Err(serde::de::Error::custom(format!(
                "Invalid Value length: {} bytes (expected 32 for Hash256)",
                bytes.len()
            )));
        }

        // For spec tests, we use the bytes directly as the hash instead of hashing them
        // This is because the QBFT message root field should contain these exact bytes
        // which matches what the Go implementation puts in the root field
        Ok(Hash256::from_slice(bytes.as_slice()))
    }

    // Convert from u64 into Round
    pub(crate) fn deserialize_u64_into_round<'de, D>(
        deserializer: D,
    ) -> Result<Option<Round>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let round = <u64>::deserialize(deserializer).map_err(|e| {
            eprintln!("DEBUG: Failed to deserialize Round field as u64: {e}");
            e
        })?;

        if round == 0 {
            Ok(None)
        } else {
            Ok(Some(round.into()))
        }
    }
}
