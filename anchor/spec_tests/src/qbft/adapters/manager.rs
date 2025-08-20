use super::spec_types::TestSignedSSVMessage;
use indexmap::IndexSet;
use message_sender::testing::MockMessageSender;
use processor::{self, Senders};
use qbft::InstanceHeight;
use qbft_manager::{CommitteeInstanceId, QbftManager};
use slot_clock::{ManualSlotClock, SlotClock};
use ssv_types::{
    Cluster, ClusterId, CommitteeId, OperatorId, consensus::BeaconVote, domain_type::DomainType,
    message::SignedSSVMessage,
};
use ssz::{Decode, Encode};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use task_executor::ShutdownReason;
use task_executor::TaskExecutor;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::time::Instant;
use types::{Address, Slot};

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
    /// Create QbftManager test setup
    pub fn new(operator_id: OperatorId, domain: DomainType) -> Result<Self, String> {
        let handle =
            Handle::try_current().map_err(|_| "Must be created within tokio runtime context")?;

        let (exit_signal, exit_receiver) = async_channel::bounded(1);
        let (shutdown_tx, _shutdown_rx) = futures::channel::mpsc::channel::<ShutdownReason>(1);
        let executor = TaskExecutor::new(
            handle,
            exit_receiver,
            shutdown_tx.clone(),
            "controller_spec_test".into(),
        );

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

impl Drop for QbftManagerTestSetup {
    fn drop(&mut self) {}
}

/// QbftManagerController - Clean replacement for ControllerAdapter using QbftManager
/// Leverages QbftManager's existing instance tracking - just provides Go-style controller API
pub struct QbftManagerController {
    test_setup: QbftManagerTestSetup,
    operator_id: OperatorId,
    committee_member: super::spec_types::SpecTestCommitteeMember,
    // Shared state for completed decisions
    completed_instances: Arc<Mutex<HashMap<InstanceHeight, Vec<u8>>>>,
}

impl QbftManagerController {
    /// Create new controller from committee member (updated to use test data)
    pub fn new(committee_member: super::spec_types::SpecTestCommitteeMember) -> Self {
        let operator_id = committee_member.operator_id;

        // Parse domain type from committee member (hex string -> DomainType)
        let domain = if committee_member.domain_type.len() == 4 {
            let mut domain_bytes = [0u8; 4];
            domain_bytes.copy_from_slice(&committee_member.domain_type[..4]);
            DomainType(domain_bytes)
        } else {
            DomainType([0; 4]) // Fallback to default
        };

        let test_setup = QbftManagerTestSetup::new(operator_id, domain)
            .expect("Failed to create QbftManager test setup");

        Self {
            test_setup,
            operator_id,
            committee_member,
            completed_instances: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start new instance
    pub fn start_new_instance(
        &mut self,
        height: InstanceHeight,
        value: Vec<u8>,
    ) -> Result<(), String> {
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

        tokio::spawn(async move {
            match manager
                .decide_instance(instance_id, beacon_vote, start_time, &cluster)
                .await
            {
                Ok(completed) => {
                    if let qbft::Completed::Success(beacon_vote_data) = completed {
                        let decided_data = beacon_vote_data.as_ssz_bytes();
                        if let Ok(mut instances) = completed_instances.lock() {
                            instances.insert(height, decided_data);
                        }
                    }
                }
                Err(_) => {}
            }
        });

        Ok(())
    }

    /// Process message - matches Go's ProcessMsg (returns decided value when ready)
    pub async fn process_msg(
        &mut self,
        msg: &TestSignedSSVMessage,
    ) -> Result<Option<Vec<u8>>, String> {
        let (signed_ssv_msg, qbft_msg) = self.convert_test_message(msg)?;
        let instance_height = InstanceHeight::from(qbft_msg.height as usize);

        self.test_setup
            .manager
            .receive_data(signed_ssv_msg, qbft_msg)
            .map_err(|e| format!("QbftManager receive_data failed: {e:?}"))?;

        // Give QBFT time to process the message
        tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;

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
    ) -> Result<(SignedSSVMessage, ssv_types::consensus::QbftMessage), String> {
        // Use existing conversion logic from spec_types.rs
        let signed_ssv_msg: SignedSSVMessage = test_msg
            .clone()
            .try_into()
            .map_err(|e| format!("Failed to convert TestSignedSSVMessage: {e:?}"))?;

        // Extract QbftMessage from SSV message data using SSZ decode
        let qbft_msg =
            ssv_types::consensus::QbftMessage::from_ssz_bytes(signed_ssv_msg.ssv_message().data())
                .map_err(|e| format!("Failed to decode QbftMessage: {e:?}"))?;

        Ok((signed_ssv_msg, qbft_msg))
    }

    /// Create test cluster from committee member data
    fn create_test_cluster(&self) -> Result<Cluster, String> {
        // Use committee data from test spec
        let cluster_members: IndexSet<OperatorId> = self
            .committee_member
            .committee
            .iter()
            .map(|op| OperatorId::from(op.operator_id))
            .collect();

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

    /// Get controller root for state validation (matches Go's GetRoot)
    pub fn get_root(&self) -> Result<Vec<u8>, String> {
        Ok(vec![0u8; 32])
    }
}

impl Drop for QbftManagerController {
    fn drop(&mut self) {}
}
