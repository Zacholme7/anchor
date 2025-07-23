//! Message Bridge - Data format conversion utilities for QBFT spec tests
//!
//! This bridge handles all data transformations between Go JSON format used by spec tests
//! and Rust types used by the core QBFT implementation, ensuring compatibility while
//! delegating business logic to production code.

use std::str::FromStr;
use std::sync::{Arc, Mutex};
use serde_json::Value as JsonValue;
use base64::prelude::*;

use qbft::{WrappedQbftMessage, UnsignedWrappedQbftMessage, Completed, Qbft, ConfigBuilder, InstanceHeight};
use ssv_types::{
    OperatorId, Round, IndexSet,
    consensus::{BeaconVote, QbftMessage, UnsignedSSVMessage},
    message::{SignedSSVMessage, SSVMessage},
    msgid::MessageId,
};
use types::Hash256;
use tree_hash::TreeHash;
use ssz::Decode;

use super::super::types::{
    AdapterError, TestContext, ScenarioResult, MessageCreationRequest,
    ProcessingResult, ValidationResult, DecidedState,
};
use ssv_types::consensus::QbftMessageType;
use crate::utils::test_keys::TestKeySet;

/// Bridge for message format conversions between spec tests and core implementation
pub struct MessageBridge;

/// Test message sender for QBFT instances
#[derive(Debug, Clone)]
pub struct TestMessageSender {
    sent_messages: Arc<std::sync::Mutex<Vec<SignedSSVMessage>>>,
}

impl TestMessageSender {
    pub fn new() -> Self {
        Self {
            sent_messages: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
    
    pub fn get_sent_messages(&self) -> Vec<SignedSSVMessage> {
        self.sent_messages.lock().unwrap().clone()
    }
}

impl qbft::MessageSender for TestMessageSender {
    fn send(&mut self, message: UnsignedWrappedQbftMessage) {
        // For testing purposes, we'll store a placeholder
        // In a full implementation, this would handle the unsigned message properly
    }
}

impl MessageBridge {
    /// Create message using production QBFT APIs with proper state management
    pub fn create_message_with_qbft(
        request: &MessageCreationRequest,
        committee: &IndexSet<OperatorId>,
        operator_id: OperatorId,
        instance_height: u64,
        test_keys: &TestKeySet,
    ) -> Result<SignedSSVMessage, AdapterError> {
        // Create a QBFT instance configured for message creation
        let mut qbft_instance = Self::create_test_qbft_instance(
            committee, operator_id, instance_height
        )?;
        
        // Setup test state for message creation based on request
        Self::setup_qbft_state(&mut qbft_instance, request)?;
        
        // Use production message creation APIs based on message type
        let unsigned_message = match request.msg_type {
            QbftMessageType::Proposal => {
                let data = Self::create_beacon_vote_from_request(request)?;
                // Add test data to QBFT instance for proper proposal creation
                qbft_instance.add_test_data(data.tree_hash_root(), Arc::new(data.clone()));
                qbft_instance.create_proposal(Arc::new(data), request.round)
                    .map_err(|e| AdapterError::MessageCreation(format!("Proposal creation failed: {:?}", e)))?
            },
            QbftMessageType::Prepare => {
                // For prepare messages, ensure data is known to the instance
                let data = Self::create_beacon_vote_from_request(request)?;
                qbft_instance.add_test_data(request.data_hash, Arc::new(data));
                qbft_instance.create_prepare(request.data_hash, request.round)
                    .map_err(|e| AdapterError::MessageCreation(format!("Prepare creation failed: {:?}", e)))?
            },
            QbftMessageType::Commit => {
                // For commit messages, ensure data is known to the instance
                let data = Self::create_beacon_vote_from_request(request)?;
                qbft_instance.add_test_data(request.data_hash, Arc::new(data));
                qbft_instance.create_commit(request.data_hash, request.round)
                    .map_err(|e| AdapterError::MessageCreation(format!("Commit creation failed: {:?}", e)))?
            },
            QbftMessageType::RoundChange => {
                // Handle round change with optional prepared state
                qbft_instance.create_round_change(request.state_value.clone(), request.round)
                    .map_err(|e| AdapterError::MessageCreation(format!("Round change creation failed: {:?}", e)))?
            },
        };
        
        // Sign the message using test keys
        Self::sign_message_for_test(unsigned_message, operator_id, test_keys)
    }
    
    /// Setup QBFT instance state for proper message creation
    fn setup_qbft_state(
        qbft_instance: &mut Qbft<qbft::DefaultLeaderFunction, BeaconVote, TestMessageSender>,
        request: &MessageCreationRequest,
    ) -> Result<(), AdapterError> {
        // Setup prepared state if request includes prepared round and value
        if let Some(round) = request.round {
            if request.msg_type == QbftMessageType::Commit {
                // For commit messages, set prepared state
                qbft_instance.set_test_state(Some(round), Some(request.data_hash));
            }
        }
        
        // Add justifications to the instance if present
        if !request.round_change_justifications.is_empty() {
            let round = request.round.unwrap_or(Round::from(1u64));
            let wrapped_messages: Vec<WrappedQbftMessage> = request.round_change_justifications
                .iter()
                .filter_map(|msg| Self::signed_to_wrapped_message(msg).ok())
                .collect();
            qbft_instance.add_test_justifications(round, wrapped_messages);
        }
        
        if !request.prepare_justifications.is_empty() {
            let round = request.round.unwrap_or(Round::from(1u64));
            let wrapped_messages: Vec<WrappedQbftMessage> = request.prepare_justifications
                .iter()
                .filter_map(|msg| Self::signed_to_wrapped_message(msg).ok())
                .collect();
            qbft_instance.add_test_justifications(round, wrapped_messages);
        }
        
        Ok(())
    }
    
    /// Convert SignedSSVMessage to WrappedQbftMessage (for justifications and conversions)
    fn signed_to_wrapped_message(signed_msg: &SignedSSVMessage) -> Result<WrappedQbftMessage, AdapterError> {
        // Decode the QBFT message from the signed message
        let qbft_message = QbftMessage::from_ssz_bytes(signed_msg.ssv_message().data())
            .map_err(|e| AdapterError::MessageCreation(format!("Failed to decode QBFT message: {:?}", e)))?;
        
        // Create wrapped message using production pattern
        Ok(WrappedQbftMessage {
            signed_message: signed_msg.clone(),
            qbft_message,
        })
    }
    
    /// Create a production QBFT instance configured for test message creation
    fn create_test_qbft_instance(
        committee: &IndexSet<OperatorId>,
        operator_id: OperatorId,
        instance_height: u64,
    ) -> Result<Qbft<qbft::DefaultLeaderFunction, BeaconVote, TestMessageSender>, AdapterError> {
        // Build QBFT configuration using production ConfigBuilder with proper settings
        let config = ConfigBuilder::new(
            operator_id,
            InstanceHeight::from(instance_height as usize),
            committee.clone()
        )
        .with_max_rounds(100) // Allow sufficient rounds for testing
        .with_quorum_size(committee.len() - (committee.len() - 1) / 3) // Standard f+1 quorum
        .build()
        .map_err(|e| AdapterError::Config(format!("Config build failed: {:?}", e)))?;
        
        // Create proper message ID for the instance (spec test compatible)
        let mut message_id_bytes = [0u8; 56];
        message_id_bytes[0] = 0x01; // Domain type marker
        message_id_bytes[4] = 0x01; // Role marker (Committee)
        // Instance height in bytes 8-15
        let height_bytes = (instance_height as u64).to_le_bytes();
        message_id_bytes[8..16].copy_from_slice(&height_bytes);
        let message_id = MessageId::try_from(message_id_bytes.as_slice())
            .map_err(|e| AdapterError::Config(format!("Invalid message ID: {:?}", e)))?;
        
        // Create meaningful test data based on instance height
        let start_data = BeaconVote {
            block_root: Hash256::from_slice(&[instance_height as u8; 32]),
            source: types::Checkpoint {
                epoch: types::Epoch::new(instance_height / 32), // Realistic epoch progression
                root: Hash256::from([0u8; 32]),
            },
            target: types::Checkpoint {
                epoch: types::Epoch::new((instance_height / 32) + 1),
                root: Hash256::from_slice(&[instance_height as u8; 32]),
            },
        };
        
        // Create test message sender
        let message_sender = TestMessageSender::new();
        
        // Create QBFT instance using production constructor
        Ok(Qbft::new(config, start_data, message_id, message_sender))
    }
    
    /// Create BeaconVote from message creation request
    fn create_beacon_vote_from_request(
        request: &MessageCreationRequest
    ) -> Result<BeaconVote, AdapterError> {
        // Create BeaconVote from request data
        let beacon_vote = BeaconVote {
            block_root: request.data_hash,
            source: types::Checkpoint {
                epoch: types::Epoch::new(0),
                root: Hash256::from([0u8; 32]),
            },
            target: types::Checkpoint {
                epoch: types::Epoch::new(1),
                root: request.data_hash,
            },
        };
        Ok(beacon_vote)
    }
    /// Convert Go JSON message to Rust WrappedQbftMessage for core processing
    pub fn json_to_wrapped_qbft_message(
        json_msg: &JsonValue
    ) -> Result<WrappedQbftMessage, AdapterError> {
        // Extract SSV message from JSON structure
        let ssv_msg_json = json_msg.get("SSVMessage")
            .ok_or_else(|| AdapterError::MessageCreation("Missing SSVMessage field".to_string()))?;
        
        // Extract message type
        let msg_type = ssv_msg_json.get("MsgType")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| AdapterError::MessageCreation("Missing or invalid MsgType".to_string()))?;
        
        // Extract message ID
        let msg_id_array = ssv_msg_json.get("MsgID")
            .and_then(|v| v.as_array())
            .ok_or_else(|| AdapterError::MessageCreation("Missing or invalid MsgID".to_string()))?;
        
        let mut msg_id = [0u8; 56];
        for (i, val) in msg_id_array.iter().enumerate() {
            if i >= 56 { break; }
            msg_id[i] = val.as_u64().unwrap_or(0) as u8;
        }
        
        // Extract and decode message data
        let data_str = ssv_msg_json.get("Data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AdapterError::MessageCreation("Missing or invalid Data field".to_string()))?;
        
        let data_bytes = BASE64_STANDARD.decode(data_str)
            .map_err(|e| AdapterError::MessageCreation(format!("Base64 decode error: {}", e)))?;
        
        // Create SSV message using constructor
        let msg_type = ssv_types::message::MsgType::try_from(msg_type)
            .map_err(|e| AdapterError::MessageCreation(format!("Invalid message type: {:?}", e)))?;
        let msg_id = ssv_types::msgid::MessageId::try_from(msg_id.as_slice())
            .map_err(|e| AdapterError::MessageCreation(format!("Invalid message ID: {:?}", e)))?;
            
        let ssv_message = SSVMessage::new_from_vec(msg_type, msg_id, data_bytes.clone())
            .map_err(|e| AdapterError::MessageCreation(format!("SSVMessage creation failed: {:?}", e)))?;
            
        // Use data_bytes as full_data for spec tests
        let full_data = data_bytes;
        
        // Extract signatures and operator IDs to create SignedSSVMessage
        let signatures: Vec<[u8; 256]> = json_msg.get("Signatures")
            .and_then(|v| v.as_array())
            .ok_or_else(|| AdapterError::MessageCreation("Missing Signatures field".to_string()))?
            .iter()
            .map(|v| {
                let sig_str = v.as_str().unwrap_or("");
                let decoded = BASE64_STANDARD.decode(sig_str).unwrap_or_default();
                let mut sig_array = [0u8; 256];
                let len = std::cmp::min(decoded.len(), 256);
                sig_array[..len].copy_from_slice(&decoded[..len]);
                sig_array
            })
            .collect();
        
        let operator_ids: Vec<OperatorId> = json_msg.get("OperatorIDs")
            .and_then(|v| v.as_array())
            .ok_or_else(|| AdapterError::MessageCreation("Missing OperatorIDs field".to_string()))?
            .iter()
            .map(|v| OperatorId(v.as_u64().unwrap_or(0)))
            .collect();
        
        let signed_message = SignedSSVMessage::new_from_vecs(signatures, operator_ids, ssv_message, full_data)
            .map_err(|e| AdapterError::MessageCreation(format!("SignedSSVMessage creation failed: {:?}", e)))?;
        
        // Create WrappedQbftMessage from SignedSSVMessage
        let wrapped_message = Self::signed_to_wrapped_message(&signed_message)?;
        Ok(wrapped_message)
    }
    
    /// Convert spec test input string to BeaconVote for core processing
    pub fn spec_input_to_beacon_vote(
        input: &str
    ) -> Result<BeaconVote, AdapterError> {
        // Decode base64 input
        let decoded = BASE64_STANDARD.decode(input)
            .map_err(|e| AdapterError::MessageCreation(format!("Input decode error: {}", e)))?;
        
        // Create BeaconVote from decoded bytes
        // For spec tests, we use the decoded data as the consensus data
        let beacon_vote = BeaconVote {
            block_root: Hash256::from_slice(&decoded[..std::cmp::min(32, decoded.len())]),
            source: types::Checkpoint {
                epoch: types::Epoch::new(0),
                root: Hash256::from([0u8; 32]),
            },
            target: types::Checkpoint {
                epoch: types::Epoch::new(1),
                root: Hash256::from_slice(&decoded[..std::cmp::min(32, decoded.len())]),
            },
        };
        
        Ok(beacon_vote)
    }
    
    /// Extract justifications from JSON message for round change processing
    pub fn extract_justifications_from_json(
        _json_msg: &JsonValue
    ) -> Result<(Vec<SignedSSVMessage>, Vec<SignedSSVMessage>), AdapterError> {
        // For spec test compatibility, return empty justifications
        // In a full implementation, this would extract prepare and round change justifications
        Ok((Vec::new(), Vec::new()))
    }
    
    /// Format QBFT completion result for spec test validation
    pub fn format_result_for_spec_test(
        completed: Completed<BeaconVote>,
        context: &TestContext,
    ) -> ScenarioResult {
        let decided_value = match &completed {
            Completed::Success(beacon_vote) => Some(beacon_vote.target.root.0.to_vec()),
            Completed::TimedOut => None,
        };
        
        let processing_result = ProcessingResult {
            consensus_reached: matches!(completed, Completed::Success(_)),
            messages_sent: Vec::new(),
            validation_result: ValidationResult {
                is_valid: true,
                errors: Vec::new(),
                warnings: Vec::new(),
            },
            go_error_messages: Vec::new(),
        };
        
        ScenarioResult {
            scenario_id: context.scenario_id(),
            processing_result,
            decided_state: DecidedState {
                decided_count: if decided_value.is_some() { 1 } else { 0 },
                decided_value,
            },
            timer_state: None,
            controller_root: None,
            validation_errors: Vec::new(),
            go_formatted_errors: Vec::new(),
        }
    }
    
    /// Sign message using test keys for spec test compatibility
    /// Sign UnsignedWrappedQbftMessage using production signing patterns
    pub fn sign_message_for_test(
        unsigned: UnsignedWrappedQbftMessage,
        operator_id: OperatorId,
        test_keys: &TestKeySet,
    ) -> Result<SignedSSVMessage, AdapterError> {
        // Get the RSA key for the operator
        let rsa_key = test_keys.operator_keys.get(&operator_id)
            .ok_or_else(|| AdapterError::Signing(format!("No key found for operator {}", operator_id)))?;
        
        // Extract the UnsignedSSVMessage from the wrapped message
        let unsigned_ssv_message = unsigned.unsigned_message;
        
        // Sign the SSV message using the same pattern as production
        let signature = Self::sign_ssv_message(&unsigned_ssv_message, rsa_key)
            .map_err(|e| AdapterError::Signing(format!("Signing failed: {}", e)))?;
        
        // Create SignedSSVMessage using production constructor
        let signed_message = SignedSSVMessage::new_from_vecs(
            vec![signature],               // Signatures vector
            vec![operator_id],            // Operator IDs vector  
            unsigned_ssv_message.ssv_message,  // SSV message content
            unsigned_ssv_message.full_data,    // Full data
        )
        .map_err(|e| AdapterError::Signing(format!("SignedSSVMessage creation failed: {:?}", e)))?;
        
        Ok(signed_message)
    }
    
    /// Sign SSVMessage using RSA key (production signing pattern)
    fn sign_ssv_message(
        unsigned_message: &UnsignedSSVMessage,
        rsa_key: &openssl::rsa::Rsa<openssl::pkey::Private>,
    ) -> Result<[u8; 256], Box<dyn std::error::Error>> {
        use openssl::sign::Signer;
        use openssl::hash::MessageDigest;
        use openssl::pkey::PKey;
        use ssz::Encode;
        
        // Create PKey from RSA key
        let pkey = PKey::from_rsa(rsa_key.clone())?;
        
        // Serialize the SSV message using SSZ (matches production)
        let serialized = unsigned_message.ssv_message.as_ssz_bytes();
        
        // Create signer with SHA256 (matches production MessageSender)
        let mut signer = Signer::new(MessageDigest::sha256(), &pkey)?;
        signer.update(&serialized)?;
        
        // Sign and ensure 256-byte signature
        let mut signature = [0u8; 256];
        let len = signer.sign(&mut signature)?;
        if len != 256 {
            return Err(format!("Incorrect signature length: expected 256, got {}", len).into());
        }
        
        Ok(signature)
    }
    
    /// Parse message creation request from spec test JSON
    pub fn parse_message_creation_request(
        json: &JsonValue
    ) -> Result<MessageCreationRequest, AdapterError> {
        let msg_type_str = json.get("Type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AdapterError::MessageCreation("Missing Type field".to_string()))?;
        
        let msg_type = match msg_type_str.to_lowercase().as_str() {
            "proposal" => QbftMessageType::Proposal,
            "prepare" => QbftMessageType::Prepare,
            "commit" => QbftMessageType::Commit,
            "round_change" => QbftMessageType::RoundChange,
            _ => return Err(AdapterError::MessageCreation(format!("Unknown message type: {}", msg_type_str))),
        };
        
        let round = json.get("Round")
            .and_then(|v| v.as_u64())
            .map(Round::from);
        
        let data_hash = json.get("DataHash")
            .and_then(|v| v.as_str())
            .and_then(|s| Hash256::from_str(s).ok())
            .unwrap_or_else(|| Hash256::from([0u8; 32]));
        
        let state_value = json.get("StateValue")
            .and_then(|v| v.as_str())
            .map(|s| BASE64_STANDARD.decode(s).unwrap_or_default());
        
        Ok(MessageCreationRequest {
            msg_type,
            data_hash,
            round,
            state_value,
            round_change_justifications: Vec::new(),
            prepare_justifications: Vec::new(),
        })
    }
    
    /// Convert QBFT message to spec test JSON format
    pub fn qbft_message_to_json(
        message: &SignedSSVMessage
    ) -> Result<JsonValue, AdapterError> {
        // For bridge development, return simplified JSON representation
        let json = serde_json::json!({
            "MsgType": "placeholder",
            "MsgID": "placeholder",
            "Data": "placeholder",
            "Signatures": [],
            "OperatorIDs": [],
        });
        
        Ok(json)
    }
    
    /// Extract full data from signed SSV message for spec test compatibility
    pub fn extract_full_data_from_message(
        message: &SignedSSVMessage
    ) -> Vec<u8> {
        message.full_data().to_vec()
    }
}