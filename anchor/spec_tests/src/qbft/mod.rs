mod controller;
mod create_message;
mod message_processing;
mod qbft_message;
mod round_robin;
mod timeout;

pub use create_message::CreateMessageTest;
use qbft::{
    test_adapter::{QbftTestAdapter, TestScenario},
    TestConfig,
};
use serde::{Deserialize, Deserializer};
use ssv_types::{
    IndexSet, OperatorId, Round,
    consensus::QbftMessageType,
    message::SignedSSVMessage,
    msgid::MessageId,
};
pub use timeout::TimeoutTest;
use tree_hash::TreeHash;
use types::Hash256;

// Wrapper type around the test adapter for spec testing
pub struct SpecQbft(pub QbftTestAdapter);

impl SpecQbft {
    /// Create a new SpecQbft using the test adapter
    pub fn new(committee: IndexSet<OperatorId>, identifier: MessageId) -> Self {
        let test_config = TestConfig {
            committee_size: committee.len(),
            quorum_threshold: (committee.len() * 2 / 3) + 1,
            max_rounds: 100,
            instance_height: 0,
        };
        
        let adapter = QbftTestAdapter::new(committee, identifier, test_config)
            .expect("Valid test adapter configuration");
        
        SpecQbft(adapter)
    }

    /// Set the signing key for the test adapter
    pub fn set_signing_key(&mut self, key: openssl::pkey::PKey<openssl::pkey::Private>) {
        self.0.set_signing_key(key);
    }

    /// Set up a test scenario for the adapter
    pub fn setup_test_scenario(&mut self, scenario: TestScenario) -> Result<(), qbft::TestError> {
        self.0.setup_test_scenario(scenario)
    }

    /// Create a message using the core QBFT spec method with proper state setup
    pub fn create_message(
        &mut self,
        message_type: QbftMessageType,
        data_hash: Hash256,
        round: Option<Round>,
        state_value: Option<Vec<u8>>,
        round_change_justifications: Vec<SignedSSVMessage>,
        prepare_justifications: Vec<SignedSSVMessage>,
    ) -> Result<SignedSSVMessage, qbft::TestError> {
        use sha2::{Sha256, Digest};
        
        // For round change messages, determine the correct effective_data_hash based on StateValue
        // This follows the working implementation logic:
        // 1. If StateValue is provided, use SHA256(StateValue) as the data_hash (previously prepared)
        // 2. If StateValue is None, use zero hash (not previously prepared)
        let effective_data_hash = if message_type == QbftMessageType::RoundChange {
            if let Some(ref state_value_bytes) = state_value {
                // Use SHA256 of the StateValue as the data_hash for previously prepared round change
                Hash256::from_slice(&Sha256::digest(state_value_bytes))
            } else {
                // Use zero hash for non-prepared round change
                Hash256::default()
            }
        } else {
            data_hash
        };
        
        // Create the unsigned message using the core QBFT logic
        let mut unsigned_message = self.0.new_unsigned_message_spec(
            message_type,
            effective_data_hash,
            round_change_justifications,
            prepare_justifications,
            round,
        );
        
        // Handle round change state value full_data
        if message_type == QbftMessageType::RoundChange {
            if let Some(state_value_bytes) = state_value {
                unsigned_message.unsigned_message.full_data = state_value_bytes;
            }
        }
        
        // Sign the message
        self.0.sign_message(unsigned_message)
    }

    /// Verify that the message root matches the expected value
    pub fn verify_root(&self, msg: SignedSSVMessage, root: Hash256) -> bool {
        msg.tree_hash_root() == root
    }
}

#[derive(Eq, PartialEq, Hash, Debug)]
pub(crate) enum QbftSpecTestType {
    Timeout,
    QbftMessage,
    MessageProcessing,
    CreateMessage,
    Controller,
    RoundRobin,
}

// Contains specific identifier for the test file
impl std::fmt::Display for QbftSpecTestType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            QbftSpecTestType::Timeout => write!(f, "timeout"),
            QbftSpecTestType::QbftMessage => write!(f, "MsgSpecTest"),
            QbftSpecTestType::MessageProcessing => write!(f, "MsgProcessingSpecTest"),
            QbftSpecTestType::CreateMessage => write!(f, "CreateMsgSpecTest"),
            QbftSpecTestType::Controller => write!(f, "ControllerSpecTest"),
            QbftSpecTestType::RoundRobin => write!(f, "RoundRobinSpecTest"),
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
