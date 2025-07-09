mod controller;
mod create_message;
mod field_validator;
mod message_processing;
mod qbft_message;
mod round_robin;
mod timeout;

use std::{collections::VecDeque, sync::Arc};

pub use create_message::CreateMessageTest;
use base64;
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private},
    sign::Signer,
};
use parking_lot::RwLock;
use qbft::{
    Config, ConfigBuilder, DefaultLeaderFunction, InstanceHeight, Qbft, UnsignedWrappedQbftMessage,
};
use serde::{Deserialize, Deserializer};
use ssv_types::{
    IndexSet, OperatorId, Round,
    consensus::{BeaconVote, QbftMessage, QbftMessageType},
    message::SignedSSVMessage,
    msgid::MessageId,
};
use ssz::{Encode, Decode};
pub use timeout::TimeoutTest;
use tree_hash::TreeHash;
use types::Hash256;
use sha2::{Sha256, Digest};

/// Marshal justifications by creating SSZ bytes for each message without full_data.
/// This replicates the Go behavior where MarshalJustifications calls WithoutFullData()
/// before marshaling each message.
pub fn marshal_justifications(messages: &[SignedSSVMessage]) -> Result<Vec<Vec<u8>>, String> {
    let mut result = Vec::new();
    
    for message in messages {
        // Use the existing encode_without_full_data method which matches Go's behavior
        let marshaled = message.encode_without_full_data();
        result.push(marshaled);
    }
    
    Ok(result)
}

// Convenient type wrapper
pub type QbftSendFn = Box<dyn FnMut(UnsignedWrappedQbftMessage) + Send + Sync>;
pub type ExplicitQbft = Qbft<DefaultLeaderFunction, BeaconVote, QbftSendFn>;
pub type ExplicitSendFn = Arc<RwLock<VecDeque<UnsignedWrappedQbftMessage>>>;

// Wrapper type around a QBFT instance that allows us to crate spec testing specific functions
pub struct SpecQbft(pub ExplicitQbft);
/// Extract proposal data from justifications if available.
/// Returns the full_data from the first justification that contains data.
fn extract_proposal_data_from_justifications(
    round_change_justifications: &[SignedSSVMessage],
    prepare_justifications: &[SignedSSVMessage],
) -> Option<Vec<u8>> {
    // Check round change justifications first
    for justification in round_change_justifications {
        if !justification.full_data().is_empty() {
            return Some(justification.full_data().to_vec());
        }
    }
    
    // Check prepare justifications
    for justification in prepare_justifications {
        if !justification.full_data().is_empty() {
            return Some(justification.full_data().to_vec());
        }
    }
    
    None
}

impl SpecQbft {
    // Construct a wrapped qbft instance
    pub fn new(committee: IndexSet<OperatorId>, identifier: MessageId) -> Self {
        let config: Config<DefaultLeaderFunction> =
            ConfigBuilder::new(1.into(), InstanceHeight::default(), committee)
                .build()
                .unwrap();

        // Todo!(). For creation tests, start data does not matter since we are not testing
        // consensus. Adjust for consensus tests
        let data = BeaconVote {
            block_root: Hash256::random(),
            source: types::Checkpoint::default(),
            target: types::Checkpoint::default(),
        };

        let msg_queue = Arc::new(RwLock::new(VecDeque::new()));
        let msg_queue_clone = msg_queue.clone();

        let message_handler: QbftSendFn = Box::new(move |message| {
            msg_queue_clone.write().push_back(message);
        });

        let qbft = Qbft::new(config, data, identifier, message_handler);

        SpecQbft(qbft)
    }

    // Create a new UnsignedSSVMessage. Will be send to the queue registered with the qbft instance
    pub fn create_message(
        &self,
        message_type: QbftMessageType,
        data_hash: Hash256,
        round: Option<Round>,
        round_change_justifications: Vec<SignedSSVMessage>,
        prepare_justifications: Vec<SignedSSVMessage>,
    ) -> UnsignedWrappedQbftMessage {
        self.create_message_with_state_value(
            message_type,
            data_hash,
            round,
            round_change_justifications,
            prepare_justifications,
            None,
        )
    }
    
    // Create a new UnsignedSSVMessage with optional StateValue for round change messages
    pub fn create_message_with_state_value(
        &self,
        message_type: QbftMessageType,
        data_hash: Hash256,
        round: Option<Round>,
        round_change_justifications: Vec<SignedSSVMessage>,
        prepare_justifications: Vec<SignedSSVMessage>,
        state_value: Option<&str>,
    ) -> UnsignedWrappedQbftMessage {
        println!("\n🏗️ QBFT CREATE_MESSAGE:");
        println!("Message Type: {:?}", message_type);
        println!("Data Hash: {}", hex::encode(data_hash));
        println!("Round: {:?}", round);
        println!("RC Justifications: {}", round_change_justifications.len());
        println!("Prep Justifications: {}", prepare_justifications.len());
        
        // For round change messages, check if we should use the "previously prepared" logic
        // The logic depends ONLY on StateValue:
        // 1. If StateValue is provided, use SHA256(StateValue) as the root (previously prepared)
        // 2. If StateValue is None, use zero root (not previously prepared)
        // PrepareJustifications are marshaled into the message but don't affect the root
        let effective_data_hash = if matches!(message_type, QbftMessageType::RoundChange) {
            if let Some(state_value_str) = state_value {
                println!("🔄 RoundChange with StateValue - using SHA256(StateValue) as root (previously prepared)");
                if let Ok(state_value_bytes) = base64::decode(state_value_str) {
                    println!("StateValue decoded: {} bytes, hex: {}", state_value_bytes.len(), hex::encode(&state_value_bytes));
                    // Use SHA256 of the StateValue as the root
                    Hash256::from_slice(&Sha256::digest(&state_value_bytes))
                } else {
                    println!("❌ Failed to decode StateValue as base64, using zero root");
                    Hash256::default()
                }
            } else {
                println!("🔄 RoundChange without StateValue - using zero root (not previously prepared)");
                // Use zero hash for non-prepared round change (regardless of justifications)
                Hash256::default()
            }
        } else {
            data_hash
        };
        
        // For proposals, determine the correct full_data based on justifications
        let _proposal_data = if matches!(message_type, QbftMessageType::Proposal) {
            // If justifications contain full_data, use it (this is the 27-byte Go test data)
            println!("🔍 Extracting proposal data from justifications...");
            let data = extract_proposal_data_from_justifications(
                &round_change_justifications,
                &prepare_justifications,
            );
            if let Some(ref data) = data {
                println!("✅ Extracted {} bytes of proposal data", data.len());
                println!("Data hex: {}", hex::encode(data));
            } else {
                println!("❌ No proposal data found in justifications");
            }
            data
        } else {
            println!("ℹ️ Not a proposal, no justification data extraction needed");
            None
        };

        println!("📞 Calling QBFT new_unsigned_message_spec...");
        let mut message = self.0.new_unsigned_message_spec(
            message_type,
            effective_data_hash,
            round_change_justifications,
            prepare_justifications,
            round,
        );

        println!("✅ Got unsigned message from QBFT");
        println!("Initial full_data size: {} bytes", message.unsigned_message.full_data.len());

        // Set the correct full_data based on message type and justifications
        if matches!(message_type, QbftMessageType::Proposal) {
            println!("🎯 Setting full_data for Proposal message...");
            // IMPORTANT: To match Go behavior, use the Value field (32 bytes) for proposals
            // instead of extracting from justifications. This is what Go does in CreateProposal.
            println!("Using Value field as full_data (to match Go): {} bytes", data_hash.as_slice().len());
            message.unsigned_message.full_data = data_hash.as_slice().to_vec();
        } else if matches!(message_type, QbftMessageType::RoundChange) {
            // For round change messages, use StateValue as full_data if it exists
            if let Some(state_value_str) = state_value {
                if let Ok(state_value_bytes) = base64::decode(state_value_str) {
                    println!("🔄 Setting full_data to StateValue for RoundChange: {} bytes", state_value_bytes.len());
                    message.unsigned_message.full_data = state_value_bytes;
                } else {
                    println!("🔄 Setting empty full_data for RoundChange (StateValue decode failed)");
                    message.unsigned_message.full_data = Vec::new();
                }
            } else {
                println!("🔄 Setting empty full_data for RoundChange (no StateValue)");
                message.unsigned_message.full_data = Vec::new();
            }
        } else {
            // For other non-proposal messages, leave full_data empty
            println!("🔄 Setting empty full_data for non-proposal message");
            message.unsigned_message.full_data = Vec::new();
        };
        
        println!("Final full_data size: {} bytes", message.unsigned_message.full_data.len());
        if !message.unsigned_message.full_data.is_empty() {
            println!("Final full_data hex: {}", hex::encode(&message.unsigned_message.full_data));
        }

        message
    }

    // In favor of not having to construct an entire NetworkMessageSender, just copy the signing
    // code
    fn sign(
        &self,
        unsigned: UnsignedWrappedQbftMessage,
        private_key: &PKey<Private>,
    ) -> SignedSSVMessage {
        println!("\n🔐 SIGNING PROCESS:");
        println!("SSV Message to sign: {} bytes", unsigned.unsigned_message.ssv_message.as_ssz_bytes().len());
        println!("Full data to include: {} bytes", unsigned.unsigned_message.full_data.len());
        
        let serialized = unsigned.unsigned_message.ssv_message.as_ssz_bytes();
        println!("Serialized SSV message for signing: {} bytes", serialized.len());
        println!("Serialized hex (first 50 bytes): {}", hex::encode(&serialized[..serialized.len().min(50)]));
        
        // For spec tests, use deterministic signing to match Go test utilities behavior
        // Go test utilities use rsa.SignPKCS1v15(nil, ...) which is deterministic
        let signature = if cfg!(test) || std::env::var("SPEC_TEST_MODE").is_ok() {
            self.sign_deterministic_for_spec_tests(&serialized, private_key).expect("Signature is valid")
        } else {
            // Use normal non-deterministic signing for production
            let mut signer = Signer::new(MessageDigest::sha256(), private_key).expect("Valid signer");
            signer.update(&serialized).expect("Serialized data is valid");
            signer.sign_to_vec().expect("Signature is valid")
        };
        
        println!("Generated signature: {} bytes", signature.len());
        println!("Signature hex (first 20 bytes): {}", hex::encode(&signature[..signature.len().min(20)]));

        println!("🏗️ Creating SignedSSVMessage with:");
        println!("  Signatures: 1 signature of {} bytes", signature.len());
        println!("  OperatorIds: [1]");
        println!("  SSVMessage: {} bytes", unsigned.unsigned_message.ssv_message.as_ssz_bytes().len());
        println!("  FullData: {} bytes", unsigned.unsigned_message.full_data.len());

        let result = SignedSSVMessage::new_from_vecs(
            vec![signature.try_into().expect("Signature should be 256 bytes")],
            vec![OperatorId::from(1)], // todo!() do we pass this in??
            unsigned.unsigned_message.ssv_message,
            unsigned.unsigned_message.full_data,
        )
        .expect("Data is valid");
        
        println!("✅ SignedSSVMessage created");
        println!("Final signed message SSZ size: {} bytes", result.as_ssz_bytes().len());
        
        result
    }

    // Deterministic RSA signing for spec tests to match Go test utilities behavior
    // Go test utilities use rsa.SignPKCS1v15(nil, ...) which makes signatures deterministic
    fn sign_deterministic_for_spec_tests(
        &self,
        data: &[u8],
        private_key: &PKey<Private>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        use openssl::hash::{Hasher, MessageDigest};
        use openssl::rsa::Padding;
        
        // Calculate SHA256 hash manually
        let mut hasher = Hasher::new(MessageDigest::sha256())?;
        hasher.update(data)?;
        let hash = hasher.finish()?;
        
        // Get the RSA key from PKey
        let rsa_key = private_key.rsa()?;
        
        // Create PKCS#1 v1.5 padding manually for deterministic behavior
        // This mimics Go's rsa.SignPKCS1v15(nil, ...) behavior
        let hash_len = hash.len();
        let key_size = rsa_key.size() as usize;
        
        // PKCS#1 v1.5 padding structure: 0x00 || 0x01 || PS || 0x00 || DigestInfo || Hash
        // Where PS is padding bytes (all 0xFF) and DigestInfo is the ASN.1 structure for SHA256
        
        // SHA256 DigestInfo (ASN.1 DER encoding)
        let digest_info = &[
            0x30, 0x31, // SEQUENCE, length 49
            0x30, 0x0d, // SEQUENCE, length 13  
            0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, // SHA256 OID
            0x05, 0x00, // NULL
            0x04, 0x20, // OCTET STRING, length 32
        ];
        
        let digest_info_len = digest_info.len();
        let total_hash_len = digest_info_len + hash_len;
        
        // Calculate padding length
        let padding_len = key_size - 3 - total_hash_len; // 3 bytes for 0x00 0x01 and separator 0x00
        
        if padding_len < 8 {
            return Err("Key too small for message".into());
        }
        
        // Build the padded message deterministically
        let mut padded_msg = Vec::with_capacity(key_size);
        padded_msg.push(0x00); // Leading zero
        padded_msg.push(0x01); // Block type 01
        padded_msg.extend(std::iter::repeat(0xFF).take(padding_len)); // Padding
        padded_msg.push(0x00); // Separator
        padded_msg.extend_from_slice(digest_info); // DigestInfo
        padded_msg.extend_from_slice(&hash); // Hash
        
        // Perform raw RSA private key operation (deterministic)
        let mut signature = vec![0u8; key_size];
        let sig_len = rsa_key.private_encrypt(&padded_msg, &mut signature, Padding::NONE)?;
        signature.truncate(sig_len);
        
        Ok(signature)
    }

    // Confirm that merkle root of signed message equals the expected root
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
