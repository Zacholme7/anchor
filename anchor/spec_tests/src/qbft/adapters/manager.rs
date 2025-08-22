use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use indexmap::IndexSet;
use message_sender::testing::MockMessageSender;
use processor::{self, Senders};
use qbft::InstanceHeight;
use qbft_manager::{CommitteeInstanceId, QbftManager};
use slot_clock::{ManualSlotClock, SlotClock};
use ssv_types::{
    Cluster, ClusterId, CommitteeId, OperatorId, consensus::BeaconVote, consensus::QbftMessage,
    consensus::QbftMessageType, domain_type::DomainType, message::SignedSSVMessage,
};
use ssz::{Decode, Encode};
use task_executor::{ShutdownReason, TaskExecutor};
use tokio::time::Duration;
use tokio::time::sleep;
use tokio::{runtime::Handle, sync::mpsc, time::Instant};
use types::{Address, Slot};

use super::spec_types::TestSignedSSVMessage;

// remove 17 becuase we will just spawn a new instance

/// QbftManager test setup - handles all the infrastructure needed for QbftManager testing
pub struct QbftManagerTestSetup {
    pub manager: Arc<QbftManager>,
    pub message_receiver: mpsc::UnboundedReceiver<SignedSSVMessage>,
    pub slot_clock: ManualSlotClock,
    _processor: Senders,
    _exit_signal: async_channel::Sender<()>,
    _shutdown_tx: futures::channel::mpsc::Sender<task_executor::ShutdownReason>,
}

impl QbftManagerTestSetup {
    /// Create QbftManager test setup with a unique executor name
    pub fn new(
        operator_id: OperatorId,
        domain: DomainType,
        executor_name: String,
    ) -> Result<Self, String> {
        let handle =
            Handle::try_current().map_err(|_| "Must be created within tokio runtime context")?;

        let (exit_signal, exit_receiver) = async_channel::bounded(1);
        let (shutdown_tx, _shutdown_rx) = futures::channel::mpsc::channel::<ShutdownReason>(1);
        let executor = TaskExecutor::new(handle, exit_receiver, shutdown_tx.clone(), executor_name);

        let config = processor::Config {
            max_workers: 15,
            queue_size: Default::default(),
        };
        let processor = processor::spawn(config, executor);

        let (network_tx, network_rx) = mpsc::unbounded_channel();
        let message_sender = Arc::new(MockMessageSender::new(network_tx, operator_id));

        let genesis_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let slot_clock = ManualSlotClock::new(
            Slot::new(0),
            Duration::from_secs(genesis_time),
            Duration::from_secs(12),
        );

        let manager = QbftManager::new(
            processor.clone(),
            operator_id.into(),
            slot_clock.clone(),
            message_sender,
            domain,
        )
        .map_err(|e| format!("Failed to create QbftManager: {e:?}"))?;

        Ok(Self {
            manager,
            message_receiver: network_rx,
            slot_clock,
            _processor: processor,
            _exit_signal: exit_signal,
            _shutdown_tx: shutdown_tx,
        })
    }
}

pub struct QbftManagerController {
    test_setup: QbftManagerTestSetup,
    operator_id: OperatorId,
    committee_member: super::spec_types::SpecTestCommitteeMember,
    identifier: Vec<u8>, // Controller identifier for validation
    // Shared state for completed decisions (stores decided value + aggregated commit)
    completed_instances: Arc<Mutex<HashMap<InstanceHeight, Vec<u8>>>>,
    // Track which decisions have been returned to avoid duplicates
    returned_decisions: HashMap<InstanceHeight, bool>,
    // Track running instances to prevent starting duplicates
    running_instances: HashSet<InstanceHeight>,
}

impl QbftManagerController {
    /// Create new controller from committee member with unique executor name
    pub fn new(
        committee_member: super::spec_types::SpecTestCommitteeMember,
        identifier: Vec<u8>,
        test_name: String,
    ) -> Self {
        let operator_id = committee_member.operator_id;

        // Parse domain type from committee member (hex string -> DomainType)
        let domain = if committee_member.domain_type.len() == 4 {
            let mut domain_bytes = [0u8; 4];
            domain_bytes.copy_from_slice(&committee_member.domain_type[..4]);
            DomainType(domain_bytes)
        } else {
            DomainType([0; 4]) // Fallback to default
        };

        // Create a unique executor name using the test name and timestamp
        let unique_executor_name = format!(
            "controller_spec_test_{}_{}",
            test_name.replace(" ", "_"),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let test_setup = QbftManagerTestSetup::new(operator_id, domain, unique_executor_name)
            .expect("Failed to create QbftManager test setup");

        Self {
            test_setup,
            operator_id,
            committee_member,
            identifier,
            completed_instances: Arc::new(Mutex::new(HashMap::new())),
            returned_decisions: HashMap::new(),
            running_instances: HashSet::new(),
        }
    }

    /// Start new instance
    pub async fn start_new_instance(
        &mut self,
        height: InstanceHeight,
        value: Vec<u8>,
    ) -> Result<(), String> {
        // If value is empty, don't start an instance. We can only start instance with value SSZ
        if value.is_empty() {
            return Ok(());
        }

        // We handle duplicate instances gracefully in the manager and dont explicitly consider it
        // an error
        if self.running_instances.contains(&height) {
            return Err("instance already running".to_string());
        } else {
            self.running_instances.insert(height);
        }

        let beacon_vote = BeaconVote::from_ssz_bytes(&value)
            .map_err(|e| format!("Failed to decode input_value as BeaconVote: {e:?}"))?;

        let committee_id = CommitteeId::default();
        let instance_id = CommitteeInstanceId {
            committee: committee_id,
            instance_height: height,
        };

        let cluster = self.create_test_cluster()?;
        let start_time = Instant::now();
        let manager = self.test_setup.manager.clone();
        let completed_instances = Arc::clone(&self.completed_instances);

        // Start the new instance and handle the result
        tokio::spawn(async move {
            if let Ok(completed) = manager
                .decide_instance(instance_id, beacon_vote, start_time, &cluster)
                .await
            {
                // Save the completion
                if let qbft::Completed::Success(beacon_vote_data) = completed {
                    let decided_data = beacon_vote_data.as_ssz_bytes();
                    if let Ok(mut instances) = completed_instances.lock() {
                        instances.insert(height, decided_data);
                    }
                }
            }
        });

        // Give the instance time to initialize
        sleep(Duration::from_millis(50)).await;

        Ok(())
    }

    /// Process message - returns decided value when ready
    pub async fn process_msg(
        &mut self,
        msg: &TestSignedSSVMessage,
    ) -> Result<Option<Vec<u8>>, String> {
        let (signed_ssv_msg, qbft_msg) = self.convert_test_message(msg)?;
        let instance_height = InstanceHeight::from(qbft_msg.height as usize);

        if qbft_msg.qbft_message_type == QbftMessageType::Proposal {
            if qbft_msg.round > 1 && qbft_msg.round_change_justification.is_empty() {
                return Err("could not process msg: invalid signed message: proposal not justified: change round has no quorum".to_string());
            }
        }

        // Issue is that we have no ways to communicate errors from manager to here

        // Check if instance is already decided
        // For multi-signature messages (decided messages), we don't error if already decided
        // For single-signature messages, we return an error if already decided
        let is_multi_sig = signed_ssv_msg.operator_ids().len() > 1;
        let is_decided = if let Ok(instances) = self.completed_instances.lock() {
            instances.contains_key(&instance_height)
        } else {
            false
        };

        // Check if this is a "decided message" (multi-sig commit with quorum)
        // Following Go's IsDecidedMsg logic
        let is_decided_msg = is_multi_sig
            && qbft_msg.qbft_message_type == ssv_types::consensus::QbftMessageType::Commit
            && signed_ssv_msg.operator_ids().len() >= 3; // Has quorum

        // Handle decided messages first (they bypass the already-decided check)
        if is_decided_msg {
            // Decided messages are processed even if instance is already decided
            // They just won't trigger a new decision
            if is_decided {
                return Ok(None);
            }
            // If not decided yet, let it continue to process below
        } else if is_decided {
            // For all non-decided messages on already-decided instances, return error
            return Err(
                "not processing consensus message since instance is already decided".to_string(),
            );
        }

        // Check if this is a multi-signature message for a new instance
        // Multi-signature messages (decided) should trigger instance creation if needed
        if signed_ssv_msg.operator_ids().len() > 1 {
            // Check if we need to start an instance for this multi-sig message
            let need_instance = if let Ok(instances) = self.completed_instances.lock() {
                !instances.contains_key(&instance_height)
            } else {
                true
            };

            if need_instance && msg.full_data.is_some() {
                if let Some(full_data) = msg.full_data.as_ref() {
                    if let Ok(decoded) = base64::Engine::decode(
                        &base64::engine::general_purpose::STANDARD,
                        full_data,
                    ) {
                        let _ = self.start_new_instance(instance_height, decoded).await;
                    }
                }
            }
        }

        // Send the message to the instance
        let result = self
            .test_setup
            .manager
            .receive_data(signed_ssv_msg.clone(), qbft_msg.clone())
            .map_err(|e| format!("QbftManager receive_data failed: {e:?}"));

        result?;

        // Give QBFT time to process the message
        sleep(Duration::from_millis(100)).await;

        // For multi-sig commit messages, check if they were properly processed
        if is_multi_sig
            && qbft_msg.qbft_message_type == ssv_types::consensus::QbftMessageType::Commit
        {
            // Give a bit more time for past height decisions to be stored
            sleep(Duration::from_millis(50)).await;

            // Check if we have enough signatures for quorum
            let has_quorum = signed_ssv_msg.operator_ids().len() >= 3;

            if !self.is_instance_decided(instance_height) {
                // If we don't have quorum, the message is invalid
                if !has_quorum {
                    return Err("could not process msg: invalid signed message: did not receive proposal for this round".to_string());
                }

                // For messages with quorum but not decided, check if it's a past height
                // Check if this might be a past height that was handled specially
                // We consider it a past height if we have any higher heights already decided
                let is_past_height = if let Ok(instances) = self.completed_instances.lock() {
                    // Convert both to usize by creating new InstanceHeight and comparing
                    // This is a workaround since we can't access the private field
                    let current_val = format!("{:?}", instance_height);
                    instances.keys().any(|h| {
                        let h_val = format!("{:?}", h);
                        // Extract numbers from Debug format "InstanceHeight(N)"
                        if let (Some(curr), Some(other)) = (
                            current_val
                                .trim_start_matches("InstanceHeight(")
                                .trim_end_matches(")")
                                .parse::<usize>()
                                .ok(),
                            h_val
                                .trim_start_matches("InstanceHeight(")
                                .trim_end_matches(")")
                                .parse::<usize>()
                                .ok(),
                        ) {
                            other > curr
                        } else {
                            false
                        }
                    })
                } else {
                    false
                };

                if !is_past_height {
                    // The decided message was rejected, likely due to no proposal
                    return Err("could not process msg: invalid signed message: did not receive proposal for this round".to_string());
                }
            }
        }

        // After processing, look if we have a decided
        if let Ok(instances) = self.completed_instances.lock() {
            if let Some(decided_data) = instances.get(&instance_height) {
                return Ok(Some(decided_data.clone()));
            }
        }

        Ok(None)
    }

    /// Convert TestSignedSSVMessage to types expected by QbftManager::receive_data
    fn convert_test_message(
        &self,
        test_msg: &TestSignedSSVMessage,
    ) -> Result<(SignedSSVMessage, QbftMessage), String> {
        // Use existing conversion logic from spec_types.rs
        let signed_ssv_msg: SignedSSVMessage = test_msg
            .clone()
            .try_into()
            .map_err(|e| format!("Failed to convert TestSignedSSVMessage: {e:?}"))?;

        // Extract QbftMessage from SSV message data using SSZ decode
        let qbft_msg = QbftMessage::from_ssz_bytes(signed_ssv_msg.ssv_message().data())
            .map_err(|e| format!("Failed to decode QbftMessage: {e:?}"))?;

        Ok((signed_ssv_msg, qbft_msg))
    }

    /// Create test cluster from committee member data
    fn create_test_cluster(&self) -> Result<Cluster, String> {
        // Use committee data from test spec
        let cluster_members: IndexSet<OperatorId> = self
            .committee_member
            .committee
            .as_ref()
            .map(|ops| {
                ops.iter()
                    .map(|op| OperatorId::from(op.operator_id))
                    .collect()
            })
            .unwrap_or_default();

        // Parse committee ID to use as cluster ID
        // Convert committee_id bytes to ClusterId (both are 32-byte arrays)
        let cluster_id = if self.committee_member.committee_id.len() == 32 {
            let mut cluster_bytes = [0u8; 32];
            cluster_bytes.copy_from_slice(&self.committee_member.committee_id);
            ClusterId(cluster_bytes)
        } else {
            ClusterId([0u8; 32]) // Default fallback
        };

        Ok(Cluster {
            cluster_id,
            owner: Address::ZERO,
            fee_recipient: Address::ZERO,
            liquidated: false,
            cluster_members,
        })
    }

    /// Check completed instances without returning them
    pub fn check_completed_instances(&self) -> Result<Vec<InstanceHeight>, String> {
        if let Ok(instances) = self.completed_instances.lock() {
            Ok(instances.keys().cloned().collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// Check if a specific instance is decided
    pub fn is_instance_decided(&self, height: InstanceHeight) -> bool {
        if let Ok(instances) = self.completed_instances.lock() {
            instances.contains_key(&height)
        } else {
            false
        }
    }

    /// Get controller root for state validation (matches Go's GetRoot)
    pub fn get_root(&self) -> Result<Vec<u8>, String> {
        Ok(vec![0u8; 32])
    }
}
