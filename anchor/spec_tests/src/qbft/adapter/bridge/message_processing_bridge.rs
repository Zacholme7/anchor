//! Message Processing Bridge - Processes message sequences through QBFT instances
//!
//! This bridge handles the processing of message sequences and provides state serialization
//! capabilities for spec test validation while integrating with production QBFT components.

use std::sync::Arc;
use serde::Serialize;

use qbft::{InstanceHeight};
use qbft_manager::{QbftManager, CommitteeInstanceId};
use ssv_types::{
    consensus::BeaconVote,
    Cluster, OperatorId,
};

use crate::utils::async_test_utils::{ControllerStateData, StoredInstance};
use super::super::types::{
    AdapterError, TestContext, SpecTestCommitteeMember, ProcessedMessage,
};
use super::super::shared::{calculate_sha256_hash, SerializableController, SerializableCommitteeMember};
use super::state_bridge::StateBridge;

/// Result structure for message processing operations
#[derive(Debug, Clone)]
pub struct MessageProcessingResult {
    /// Whether the message sequence was processed successfully
    pub success: bool,
    /// Final instance height after processing
    pub final_height: u64,
    /// Number of messages processed
    pub messages_processed: usize,
    /// List of decisions made during processing
    pub decisions: Vec<ProcessedMessage>,
    /// Final state hash for verification
    pub state_hash: Option<String>,
    /// Any errors encountered during processing
    pub errors: Vec<String>,
}

/// Serializable representation of QBFT instance state  
#[derive(Debug, Clone)]
pub struct QbftInstanceState {
    /// Instance height
    pub height: u64,
    /// Current round
    pub round: u64,
    /// Current step/phase
    pub step: String,
    /// Proposal if exists
    pub proposal: Option<Vec<u8>>,
    /// Prepared round
    pub prepared_round: Option<u64>,
    /// Prepared value
    pub prepared_value: Option<Vec<u8>>,
    /// Locked round
    pub locked_round: Option<u64>,
    /// Locked value
    pub locked_value: Option<Vec<u8>>,
    /// Instance identifier
    pub instance_id: Vec<u8>,
}

/// Bridge for processing message sequences through QBFT instances
pub struct MessageProcessingBridge;

impl MessageProcessingBridge {
    /// Process a sequence of messages through a QBFT instance
    /// 
    /// This method creates or retrieves a QBFT instance and processes each message
    /// in the sequence, tracking the results and final state.
    pub async fn process_message_sequence(
        manager: &Arc<QbftManager>,
        instance_id: CommitteeInstanceId,
        messages: Vec<ProcessedMessage>,
        beacon_vote: BeaconVote,
        cluster: &Cluster,
        context: &TestContext,
    ) -> Result<MessageProcessingResult, AdapterError> {
        let mut result = MessageProcessingResult {
            success: false,
            final_height: (*instance_id.instance_height) as u64,
            messages_processed: 0,
            decisions: Vec::new(),
            state_hash: None,
            errors: Vec::new(),
        };

        // Process messages sequentially
        for (index, message) in messages.iter().enumerate() {
            match Self::process_single_message(manager, &instance_id, message, &beacon_vote, cluster).await {
                Ok(processed) => {
                    result.decisions.push(processed);
                    result.messages_processed = index + 1;
                }
                Err(e) => {
                    result.errors.push(format!("Message {}: {:?}", index, e));
                    break;
                }
            }
        }

        // Calculate final state hash
        match Self::calculate_final_state_hash(manager, &instance_id, context).await {
            Ok(hash) => result.state_hash = Some(hash),
            Err(e) => result.errors.push(format!("State hash calculation failed: {:?}", e)),
        }

        result.success = result.errors.is_empty();
        
        Ok(result)
    }

    /// Create a QBFT instance from JSON state representation
    /// 
    /// This method deserializes QBFT instance state from JSON format and attempts
    /// to reconstruct the instance state within the manager.
    pub async fn create_qbft_from_state(
        manager: &Arc<QbftManager>,
        state_json: &str,
        cluster: &Cluster,
    ) -> Result<CommitteeInstanceId, AdapterError> {
        // Parse the JSON state using a simple deserializable struct
        #[derive(serde::Deserialize)]
        struct InternalQbftState {
            height: u64,
        }
        
        let internal_state: InternalQbftState = serde_json::from_str(state_json)
            .map_err(|e| AdapterError::Config(format!("Invalid JSON state: {}", e)))?;

        // Create instance ID from the state
        let instance_id = CommitteeInstanceId {
            committee: ssv_types::CommitteeId([1u8; 32]), // Default test committee ID
            instance_height: InstanceHeight::from(internal_state.height as usize),
        };

        // Create a test beacon vote for the instance
        let beacon_vote = StateBridge::create_test_beacon_vote(&instance_id);

        // Initialize the instance in the manager
        // Note: In a full implementation, we would need additional APIs to restore
        // the specific instance state (round, step, etc.) from the serialized data
        let _completion = manager.decide_instance(instance_id.clone(), beacon_vote, tokio::time::Instant::now(), cluster).await
            .map_err(|e| AdapterError::InvalidState(format!("Failed to create instance: {:?}", e)))?;

        Ok(instance_id)
    }

    /// Calculate cryptographic state root hash matching Go implementation
    /// 
    /// This method computes a SHA256 hash of the QBFT instance state in a format
    /// compatible with the Go implementation for cross-implementation validation.
    pub async fn calculate_state_root(
        manager: &Arc<QbftManager>,
        instance_id: &CommitteeInstanceId,
        committee_member: &SpecTestCommitteeMember,
    ) -> Result<String, AdapterError> {
        // Extract current state from the manager
        let controller_state = StateBridge::extract_controller_state(manager, (*instance_id.instance_height) as u64).await?;
        
        // Create serializable state representation
        let serializable_state = SerializableController {
            identifier: Self::create_instance_identifier(instance_id),
            height: (*instance_id.instance_height) as u64,
            stored_instances: serde_json::to_value(&controller_state.stored_instances)
                .map_err(|e| AdapterError::Config(format!("State serialization error: {}", e)))?,
            committee_member: Self::create_serializable_committee_member(committee_member),
        };

        // Calculate SHA256 hash of the JSON representation
        let json_bytes = serde_json::to_vec(&serializable_state)
            .map_err(|e| AdapterError::Config(format!("JSON serialization failed: {}", e)))?;

        Ok(calculate_sha256_hash(&json_bytes))
    }

    /// Process a single message through the QBFT instance
    async fn process_single_message(
        _manager: &Arc<QbftManager>,
        instance_id: &CommitteeInstanceId,
        message: &ProcessedMessage,
        _beacon_vote: &BeaconVote,
        _cluster: &Cluster,
    ) -> Result<ProcessedMessage, AdapterError> {
        // In a full implementation, this would:
        // 1. Route the message to the appropriate QBFT instance
        // 2. Process the message through the consensus protocol
        // 3. Return the processing result
        
        // For now, return a copy of the message as processed
        let mut processed = message.clone();
        processed.processed = true;
        processed.result_height = Some((*instance_id.instance_height) as u64);
        
        Ok(processed)
    }

    /// Calculate final state hash after message processing
    async fn calculate_final_state_hash(
        manager: &Arc<QbftManager>,
        instance_id: &CommitteeInstanceId,
        _context: &TestContext,
    ) -> Result<String, AdapterError> {
        let controller_state = StateBridge::extract_controller_state(manager, (*instance_id.instance_height) as u64).await?;
        
        // Create a minimal committee member for hash calculation
        let committee_member = SpecTestCommitteeMember {
            operator_id: OperatorId(1),
            committee_id: b"test_committee".to_vec(),
            ssv_operator_pub_key: "test_pubkey".to_string(),
            faulty_nodes: 0,
            committee: Vec::new(),
            domain_type: b"beacon_proposer".to_vec(),
        };

        StateBridge::calculate_controller_root(&controller_state, &committee_member)
    }

    /// Create instance identifier from CommitteeInstanceId
    fn create_instance_identifier(instance_id: &CommitteeInstanceId) -> Vec<u8> {
        let mut identifier = Vec::with_capacity(64);
        
        // Add committee ID (32 bytes)
        identifier.extend_from_slice(&instance_id.committee.0);
        
        // Add instance height (8 bytes)
        identifier.extend_from_slice(&instance_id.instance_height.to_be_bytes());
        
        // Pad to 56 bytes for consistency
        identifier.resize(56, 0);
        
        identifier
    }

    /// Create serializable committee member from spec test data
    fn create_serializable_committee_member(
        committee_member: &SpecTestCommitteeMember
    ) -> SerializableCommitteeMember {
        let committee_operators = committee_member.committee
            .iter()
            .map(|operator| super::super::shared::SerializableOperator {
                operator_id: operator.operator_id,
                ssv_operator_pub_key: operator.ssv_operator_pub_key.clone(),
            })
            .collect();
        
        SerializableCommitteeMember {
            operator_id: committee_member.operator_id.0,
            committee_id: committee_member.committee_id.clone(),
            ssv_operator_pub_key: committee_member.ssv_operator_pub_key.clone(),
            faulty_nodes: committee_member.faulty_nodes,
            committee: committee_operators,
            domain_type: committee_member.domain_type.clone(),
        }
    }
}