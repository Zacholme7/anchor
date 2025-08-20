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
use ssz::Decode;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
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
    /// Create QbftManager test setup - uses same pattern as your previous ControllerAdapter
    pub fn new(operator_id: OperatorId, domain: DomainType) -> Result<Self, String> {
        // Step 1: Get current runtime handle
        let handle =
            Handle::try_current().map_err(|_| "Must be created within tokio runtime context")?;

        // Step 2: Create channels for executor
        let (exit_signal, exit_receiver) = async_channel::bounded(1);
        let (shutdown_tx, _shutdown_rx) =
            futures::channel::mpsc::channel::<task_executor::ShutdownReason>(1);
        let executor = TaskExecutor::new(
            handle,
            exit_receiver,
            shutdown_tx.clone(),
            "controller_spec_test".into(),
        );

        // Step 3: Set up processor
        let config = processor::Config {
            max_workers: 15,
            queue_size: Default::default(),
        };
        let processor = processor::spawn(config, executor);

        // Step 4: Set up network channel
        let (network_tx, network_rx) = mpsc::unbounded_channel();

        // Step 5: Create mock message sender
        let message_sender = Arc::new(MockMessageSender::new(network_tx, operator_id));

        // Step 6: Set up slot clock
        let genesis_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let slot_clock = ManualSlotClock::new(
            Slot::new(0),
            Duration::from_secs(genesis_time),
            Duration::from_secs(12), // 12-second slots
        );

        // Step 7: Create QbftManager
        let manager = QbftManager::new(
            processor.clone(),
            operator_id.into(),
            slot_clock.clone(),
            message_sender,
            domain, // Use domain from test data
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

/// QbftManagerController - Clean replacement for ControllerAdapter using QbftManager
/// Leverages QbftManager's existing instance tracking - just provides Go-style controller API
pub struct QbftManagerController {
    test_setup: QbftManagerTestSetup,
    operator_id: OperatorId,
    committee_member: super::spec_types::SpecTestCommitteeMember,
    // Track recent decisions for Go-style ProcessMsg API (returns decided values)
    recent_decisions: HashMap<InstanceHeight, Vec<u8>>,
    // Controller identifier for validation (matches Go)
    identifier: Vec<u8>,
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

        // Default test identifier (matches Go's TestingIdentifier)
        let mut id_bytes = [0u8; 56];
        id_bytes[0] = 1;
        id_bytes[1] = 2;
        id_bytes[2] = 3;
        id_bytes[3] = 4;

        Self {
            test_setup,
            operator_id,
            committee_member,
            recent_decisions: HashMap::new(),
            identifier: id_bytes.to_vec(),
        }
    }

    /// Start new instance - matches existing ControllerAdapter::start_new_instance signature
    pub fn start_new_instance(
        &mut self,
        height: InstanceHeight,
        value: Vec<u8>,
    ) -> Result<(), String> {
        // SSZ decode the input_value into a BeaconVote (matches Go spec test expectation)
        let beacon_vote = BeaconVote::from_ssz_bytes(&value)
            .map_err(|e| format!("Failed to decode input_value as BeaconVote: {e:?}"))?;

        // Create instance ID using committee ID from test data
        let committee_id = CommitteeId::try_from(self.committee_member.committee_id.as_slice())
            .map_err(|e| format!("Invalid committee ID: {e:?}"))?;
        let instance_id = CommitteeInstanceId {
            committee: committee_id,
            instance_height: height,
        };

        // Create committee/cluster - simplified for now
        let cluster = self.create_test_cluster()?;

        // Call QbftManager::decide_instance - this starts consensus (matches Go's StartNewInstance)
        let start_time = Instant::now();
        let manager = self.test_setup.manager.clone();

        // Spawn the decision task - QbftManager handles the rest
        tokio::spawn(async move {
            let _result = manager
                .decide_instance(instance_id, beacon_vote, start_time, &cluster)
                .await;
            // When this completes, the instance is decided - verification happens after
        });

        Ok(())
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

    /// Process message - matches Go's ProcessMsg (returns decided value when ready)
    pub fn process_msg(&mut self, msg: &TestSignedSSVMessage) -> Result<Option<Vec<u8>>, String> {
        // Convert TestSignedSSVMessage to SignedSSVMessage and QbftMessage
        let (signed_ssv_msg, qbft_msg) = self.convert_test_message(msg)?;

        // Call QbftManager::receive_data - routes message to correct instance
        self.test_setup
            .manager
            .receive_data(signed_ssv_msg, qbft_msg)
            .map_err(|e| format!("QbftManager receive_data failed: {e:?}"))?;

        // For now, return None - we'll figure out decision detection later
        // Go returns decided value when consensus reached, we'll iterate on this
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

    /// Get controller root for state validation (matches Go's GetRoot)
    pub fn get_root(&self) -> Result<Vec<u8>, String> {
        // For now, return a simple root - we'll improve this later
        // Go computes a root hash of the controller's internal state
        Ok(vec![0u8; 32])
    }

    /// Get controller identifier (matches Go's controller.Identifier)
    pub fn get_identifier(&self) -> &[u8] {
        &self.identifier
    }
}
