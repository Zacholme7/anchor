use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};

use message_sender::testing::MockMessageSender;
use ssv_types::{
    OperatorId, CommitteeId, IndexSet,
    consensus::BeaconVote,
    message::SignedSSVMessage,
};
use tokio::{
    sync::mpsc::{UnboundedReceiver, unbounded_channel},
    time::timeout,
};
use types::{Hash256, Epoch, Checkpoint};
use sha2::{Sha256, Digest};
use base64::prelude::*;

/// Async test utilities for QbftManager controller tests
/// 
/// This provides a simplified async interface for testing QBFT controller behavior
/// without requiring the full QbftManager infrastructure. It simulates the essential
/// components needed for controller state testing.
pub struct AsyncQbftTestSetup {
    /// Mock QbftManager for testing controller behavior
    pub mock_manager: Arc<MockQbftManager>,
    /// Network message receiver for intercepting messages
    pub message_receiver: UnboundedReceiver<SignedSSVMessage>,
    /// Committee configuration
    pub committee: IndexSet<OperatorId>,
    /// Current operator ID
    pub operator_id: OperatorId,
}

/// Mock QbftManager that simulates essential behavior for controller tests
pub struct MockQbftManager {
    /// Mock message sender
    message_sender: MockMessageSender,
    /// Controller state tracking
    controller_state: parking_lot::RwLock<ControllerStateData>,
    /// Committee configuration
    committee: IndexSet<OperatorId>,
    /// Current operator ID
    operator_id: OperatorId,
    /// Instance completion callbacks
    completion_callbacks: parking_lot::RwLock<HashMap<u64, tokio::sync::oneshot::Sender<Option<Vec<u8>>>>>,
}

/// Controller state data extracted from QbftManager
#[derive(Debug, Clone)]
pub struct ControllerStateData {
    pub height: u64,
    pub stored_instances: Vec<StoredInstance>,
    pub active_instances: HashMap<u64, bool>, // height -> decided
}

/// Stored instance for controller state persistence
#[derive(Debug, Clone, serde::Serialize)]
pub struct StoredInstance {
    pub height: u64,
    pub decided_value: Option<Vec<u8>>,
}

/// Instance identifier for committee consensus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommitteeInstanceId {
    pub height: u64,
    pub committee_id: CommitteeId,
}

impl AsyncQbftTestSetup {
    /// Create a new async test setup with specified committee size
    pub async fn new(committee_size: usize) -> Result<Self, QbftError> {
        if committee_size < 3 || committee_size > 13 {
            return Err(QbftError::InvalidCommitteeSize(committee_size));
        }

        // Create committee with sequential operator IDs
        let mut committee = IndexSet::new();
        for i in 1..=committee_size {
            committee.insert(OperatorId(i as u64));
        }

        let operator_id = OperatorId(1); // Default to first operator

        // Create message channels
        let (message_tx, message_receiver) = unbounded_channel();

        // Create mock message sender
        let message_sender = MockMessageSender::new(message_tx, operator_id);

        // Create mock manager
        let mock_manager = Arc::new(MockQbftManager::new(
            message_sender,
            committee.clone(),
            operator_id,
        ));

        Ok(Self {
            mock_manager,
            message_receiver,
            committee,
            operator_id,
        })
    }

    /// Start a new QBFT instance with the given input value
    pub async fn start_instance(&self, input_value: &str) -> Result<CommitteeInstanceId, QbftError> {
        let height = {
            let mut state = self.mock_manager.controller_state.write();
            state.height += 1;
            state.height
        };

        // Convert input value from base64 or string to BeaconVote
        let beacon_vote = self.convert_input_to_beacon_vote(input_value)?;
        
        // Create instance ID
        let committee_id = self.calculate_committee_id();
        let instance_id = CommitteeInstanceId {
            height,
            committee_id,
        };

        // Initialize instance state
        {
            let mut state = self.mock_manager.controller_state.write();
            state.active_instances.insert(height, false); // not decided yet
        }

        // Start the mock consensus process
        self.mock_manager.start_mock_instance(height, beacon_vote).await?;

        Ok(instance_id)
    }

    /// Process a message through the QbftManager simulation
    pub async fn process_message(&self, message: SignedSSVMessage) -> Result<(), QbftError> {
        // Validate message structure
        if message.operator_ids().is_empty() {
            return Err(QbftError::InvalidMessage("no signers".to_string()));
        }

        // Check if signers are in committee
        for operator_id in message.operator_ids() {
            if !self.committee.contains(operator_id) {
                return Err(QbftError::InvalidMessage("signer not in committee".to_string()));
            }
        }

        // Process the message through mock manager
        self.mock_manager.process_consensus_message(message).await
    }

    /// Extract current controller state for root hash calculation
    pub fn extract_controller_state(&self) -> ControllerStateData {
        self.mock_manager.controller_state.read().clone()
    }

    /// Wait for a decision on the specified instance
    pub async fn wait_for_decision(&self, instance_id: CommitteeInstanceId) -> Result<Option<Vec<u8>>, QbftError> {
        let timeout_duration = Duration::from_secs(30);
        
        timeout(timeout_duration, async {
            self.mock_manager.wait_for_instance_completion(instance_id.height).await
        })
        .await
        .map_err(|_| QbftError::Timeout)?
    }

    /// Calculate committee ID based on current committee
    fn calculate_committee_id(&self) -> CommitteeId {
        // Create a deterministic committee ID based on operator IDs
        let mut hasher = Sha256::new();
        for operator_id in &self.committee {
            hasher.update(operator_id.0.to_le_bytes());
        }
        let hash = hasher.finalize();
        CommitteeId::from(<[u8; 32]>::try_from(&hash[..32]).unwrap())
    }

    /// Convert spec test input value to BeaconVote data
    fn convert_input_to_beacon_vote(&self, input_value: &str) -> Result<BeaconVote, QbftError> {
        // Try to decode as base64 first
        let value_bytes = match BASE64_STANDARD.decode(input_value) {
            Ok(bytes) => bytes,
            Err(_) => {
                // If not base64, use raw string bytes
                input_value.as_bytes().to_vec()
            }
        };

        // Create a minimal BeaconVote for testing
        // In a real implementation, this would properly decode the consensus data
        Ok(BeaconVote {
            block_root: Hash256::from_slice(&value_bytes.get(..32).unwrap_or(&[0u8; 32])),
            source: Checkpoint {
                epoch: Epoch::new(0),
                root: Hash256::from([0u8; 32]),
            },
            target: Checkpoint {
                epoch: Epoch::new(0),
                root: Hash256::from([0u8; 32]),
            },
        })
    }
}

impl MockQbftManager {
    /// Create a new mock QBFT manager
    pub fn new(
        message_sender: MockMessageSender,
        committee: IndexSet<OperatorId>,
        operator_id: OperatorId,
    ) -> Self {
        Self {
            message_sender,
            controller_state: parking_lot::RwLock::new(ControllerStateData {
                height: 0,
                stored_instances: Vec::new(),
                active_instances: HashMap::new(),
            }),
            committee,
            operator_id,
            completion_callbacks: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    /// Start a mock QBFT instance for testing
    async fn start_mock_instance(&self, height: u64, _beacon_vote: BeaconVote) -> Result<(), QbftError> {
        // Simulate starting an instance by updating controller state
        {
            let mut state = self.controller_state.write();
            state.active_instances.insert(height, false);
        }

        // In a real implementation, this would start the actual QBFT consensus process
        // For testing, we just simulate the setup
        Ok(())
    }

    /// Process a consensus message through mock QBFT logic
    async fn process_consensus_message(&self, message: SignedSSVMessage) -> Result<(), QbftError> {
        // Decode QBFT message to get height and type
        use ssv_types::{message::MsgType, consensus::{QbftMessage, QbftMessageType}};
        use ssz::Decode;

        let ssv_msg = message.ssv_message();
        if *ssv_msg.msg_type() != MsgType::SSVConsensusMsgType {
            return Err(QbftError::InvalidMessage("not a consensus message".to_string()));
        }

        let qbft_message = QbftMessage::from_ssz_bytes(ssv_msg.data())
            .map_err(|_| QbftError::InvalidMessage("failed to decode QBFT message".to_string()))?;

        let height = qbft_message.height;

        // Check if we have an active instance for this height
        let has_active_instance = {
            let state = self.controller_state.read();
            state.active_instances.contains_key(&height)
        };

        if !has_active_instance {
            return Err(QbftError::InvalidMessage("no active instance for height".to_string()));
        }

        // Simulate consensus logic based on message type
        match qbft_message.qbft_message_type {
            QbftMessageType::Commit => {
                // For commit messages, check if we have quorum and can decide
                let quorum_threshold = (self.committee.len() * 2) / 3 + 1;
                let signer_count = message.operator_ids().len();

                if signer_count >= quorum_threshold {
                    // Simulate reaching consensus
                    let decided_value = message.full_data().to_vec();
                    self.finalize_instance(height, Some(decided_value)).await?;
                }
            }
            QbftMessageType::Proposal | QbftMessageType::Prepare | QbftMessageType::RoundChange => {
                // For other message types, just accept them
                // In a real implementation, this would update instance state
            }
        }

        Ok(())
    }

    /// Finalize an instance with a decision
    async fn finalize_instance(&self, height: u64, decided_value: Option<Vec<u8>>) -> Result<(), QbftError> {
        // Update controller state
        {
            let mut state = self.controller_state.write();
            state.active_instances.insert(height, true); // mark as decided
            
            // Add to stored instances
            state.stored_instances.push(StoredInstance {
                height,
                decided_value: decided_value.clone(),
            });
        }

        // Notify any waiting completion callbacks
        let callback = {
            let mut callbacks = self.completion_callbacks.write();
            callbacks.remove(&height)
        };

        if let Some(callback) = callback {
            let _ = callback.send(decided_value);
        }

        Ok(())
    }

    /// Wait for an instance to complete
    async fn wait_for_instance_completion(&self, height: u64) -> Result<Option<Vec<u8>>, QbftError> {
        // Check if already decided
        {
            let state = self.controller_state.read();
            if let Some(&decided) = state.active_instances.get(&height) {
                if decided {
                    // Find the stored instance
                    for stored in &state.stored_instances {
                        if stored.height == height {
                            return Ok(stored.decided_value.clone());
                        }
                    }
                }
            }
        }

        // Set up completion callback
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut callbacks = self.completion_callbacks.write();
            callbacks.insert(height, tx);
        }

        // Wait for completion
        rx.await.map_err(|_| QbftError::InstanceCancelled)
    }
}

/// Error types for QBFT async testing
#[derive(Debug, thiserror::Error)]
pub enum QbftError {
    #[error("Invalid committee size: {0}")]
    InvalidCommitteeSize(usize),
    #[error("Invalid message: {0}")]
    InvalidMessage(String),
    #[error("Instance timeout")]
    Timeout,
    #[error("Instance was cancelled")]
    InstanceCancelled,
    #[error("Base64 decode error: {0}")]
    Base64Decode(#[from] base64::DecodeError),
    #[error("Invalid state: {0}")]
    InvalidState(String),
}

impl Default for ControllerStateData {
    fn default() -> Self {
        Self {
            height: 0,
            stored_instances: Vec::new(),
            active_instances: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_qbft_setup_creation() {
        let setup = AsyncQbftTestSetup::new(4).await.unwrap();
        assert_eq!(setup.committee.len(), 4);
        assert_eq!(setup.operator_id, OperatorId(1));
    }

    #[tokio::test]
    async fn test_start_instance() {
        let setup = AsyncQbftTestSetup::new(4).await.unwrap();
        let instance_id = setup.start_instance("dGVzdCBkYXRh").await.unwrap(); // "test data" in base64
        assert_eq!(instance_id.height, 1);
    }

    #[tokio::test]
    async fn test_controller_state_extraction() {
        let setup = AsyncQbftTestSetup::new(4).await.unwrap();
        let _instance_id = setup.start_instance("dGVzdCBkYXRh").await.unwrap();
        
        let state = setup.extract_controller_state();
        assert_eq!(state.height, 1);
        assert!(state.active_instances.contains_key(&1));
    }
}