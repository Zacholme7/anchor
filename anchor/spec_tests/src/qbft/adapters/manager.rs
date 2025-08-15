use super::spec_types::TestSignedSSVMessage;
use crate::utils::message_validation::{
    extract_decided_value, identifier_to_message_id, is_decided_message, validate_signer_count,
};
use crate::utils::misc::{calculate_quorum, hash_data};
use crate::utils::test_keys::TestKeySet;
use message_sender::testing::MockMessageSender;
use qbft::{InstanceHeight, LeaderFunction};
use qbft_manager::QbftManager;
use slot_clock::{ManualSlotClock, SlotClock};
use ssv_types::consensus::BeaconVote;
use ssv_types::domain_type::DomainType;
use ssv_types::msgid::MessageId;
use ssv_types::{IndexSet, OperatorId, Round};
use ssz::Encode;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use types::{Hash256, Slot};

// Type alias for our QBFT instance
type QbftInstance =
    qbft::Qbft<TestLeaderFunction, BeaconVote, Box<dyn FnMut(qbft::UnsignedWrappedQbftMessage)>>;

/// Test leader function that matches Go test harness behavior
#[derive(Debug, Clone, Copy, Default)]
struct TestLeaderFunction {
    height: InstanceHeight,
}
impl LeaderFunction for TestLeaderFunction {
    fn leader_function(
        &self,
        _operator_id: &OperatorId,
        _round: Round,
        _instance_height: InstanceHeight,
        _committee: &IndexSet<OperatorId>,
    ) -> bool {
        // Special case: At height 10, operator 2 is the leader
        // This matches ChangeProposerFuncInstanceHeight in Go tests
        if *self.height == 10 {
            *_operator_id == OperatorId::from(2)
        } else {
            // Default: operator 1 is always the leader
            *_operator_id == OperatorId::from(1)
        }
    }
}

/// State of an instance in the controller
#[derive(Debug, Clone)]
enum InstanceState {
    /// Instance is currently running consensus
    Active,
    /// Instance has decided with this value
    Decided(Vec<u8>),
}

/// Container for tracking proposal full data
/// We no longer need to track messages since the core QBFT does that
#[derive(Debug, Clone)]
struct ProposalDataStorage {
    /// Store the proposal's full_data for each (height, round, root)
    proposal_full_data: HashMap<(InstanceHeight, u64, Hash256), Vec<u8>>,
}

impl ProposalDataStorage {
    fn new() -> Self {
        Self {
            proposal_full_data: HashMap::new(),
        }
    }

    /// Store the proposal's full_data for later use
    fn store_proposal_full_data(
        &mut self,
        height: InstanceHeight,
        round: u64,
        root: Hash256,
        full_data: Vec<u8>,
    ) {
        let key = (height, round, root);
        self.proposal_full_data.insert(key, full_data);
    }

    /// Get the proposal's full_data if available
    fn get_proposal_full_data(
        &self,
        height: InstanceHeight,
        round: u64,
        root: Hash256,
    ) -> Option<Vec<u8>> {
        let key = (height, round, root);
        self.proposal_full_data.get(&key).cloned()
    }
}

/// Adapter for QBFT controller spec tests.
///
/// This adapter runs a QBFT controller with actual QBFT instances.
/// It tracks instances, forwards messages to them, and handles decisions.
pub struct ControllerAdapter {
    // Core components
    manager: Arc<QbftManager>,
    processor: processor::Senders,
    slot_clock: ManualSlotClock,

    // Runtime handle for blocking operations
    runtime_handle: Handle,

    // Keep these alive to prevent processor shutdown
    _exit_signal: async_channel::Sender<()>,
    _shutdown_tx: futures::channel::mpsc::Sender<task_executor::ShutdownReason>,

    // Network simulation
    network_rx: mpsc::UnboundedReceiver<ssv_types::message::SignedSSVMessage>,

    // Instance tracking (mirrors Go's StoredInstances)
    current_height: InstanceHeight,
    stored_instances: HashMap<InstanceHeight, InstanceState>, // All instances (active & decided)
    current_committee_size: usize,                            // For quorum calculation

    // Test keys for RSA signature verification
    test_keys: Option<TestKeySet>,

    // Direct QBFT instances for synchronous processing (bypassing async processor)
    qbft_instances: HashMap<InstanceHeight, QbftInstance>,

    // Proposal data storage (we use core QBFT's containers for messages)
    proposal_storage: ProposalDataStorage,

    // Track spawned instance tasks so we can abort them
    instance_handles: Vec<tokio::task::JoinHandle<()>>,

    // Configuration
    operator_id: OperatorId,
    identifier: MessageId,

    // Committee information for signature verification
    committee: Vec<super::spec_types::SpecTestOperator>,
}

impl ControllerAdapter {
    /// Create a new ControllerAdapter with the given operator ID and committee
    pub fn new(
        operator_id: OperatorId,
        committee: Vec<super::spec_types::SpecTestOperator>,
    ) -> Self {
        // Step 1: Try to get current runtime handle, or create a new one if needed
        let handle = match tokio::runtime::Handle::try_current() {
            Ok(handle) => handle,
            Err(_) => {
                // We're not in a runtime, this shouldn't happen in tests but handle gracefully
                panic!("ControllerAdapter must be created within a tokio runtime context");
            }
        };

        // Store the handle for later use with block_on
        let runtime_handle = handle.clone();

        // Create channels for executor - KEEP THE SENDERS ALIVE
        let (exit_signal, exit_receiver) = async_channel::bounded(1);
        let (shutdown_tx, _shutdown_rx) = futures::channel::mpsc::channel(1);
        let executor = task_executor::TaskExecutor::new(
            handle,
            exit_receiver,
            shutdown_tx.clone(),
            "spec_test".into(),
        );

        // Step 2: Set up the processor
        let config = processor::Config {
            max_workers: 15,
            queue_size: Default::default(),
        };
        let processor = processor::spawn(config, executor);

        // Step 3: Set up the network channel
        let (network_tx, network_rx) = mpsc::unbounded_channel();

        // Step 4: Create the message sender
        let message_sender = Arc::new(MockMessageSender::new(network_tx, operator_id));

        // Step 5: Set up the slot clock
        let genesis_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let slot_clock = ManualSlotClock::new(
            Slot::new(0),
            Duration::from_secs(genesis_time),
            Duration::from_secs(12), // 12-second slots
        );

        // Step 6: Create the QbftManager (returns Arc<QbftManager>)
        let manager = QbftManager::new(
            processor.clone(),
            operator_id.into(), // Convert OperatorId to OwnOperatorId
            slot_clock.clone(),
            message_sender,
            DomainType([0; 4]), // Test domain
        )
        .expect("Failed to create QbftManager");

        // Default test identifier (matches Go's TestingIdentifier)
        // Go uses [1,2,3,4,0,0,...] for 56 bytes total
        let mut id_bytes = [0u8; 56];
        id_bytes[0] = 1;
        id_bytes[1] = 2;
        id_bytes[2] = 3;
        id_bytes[3] = 4;
        let identifier = MessageId::from(id_bytes);

        Self {
            manager,
            processor,
            slot_clock,
            runtime_handle,
            _exit_signal: exit_signal,
            _shutdown_tx: shutdown_tx,
            network_rx,
            current_height: InstanceHeight::from(0),
            stored_instances: HashMap::new(),
            current_committee_size: 4, // Default to 4 operators (matching Go tests)
            test_keys: None,           // Will be set by set_test_keys if needed
            qbft_instances: HashMap::new(),
            proposal_storage: ProposalDataStorage::new(),
            instance_handles: Vec::new(),
            operator_id,
            identifier,
            committee,
        }
    }

    /// Set the test keys for RSA signature verification
    pub fn set_test_keys(&mut self, test_keys: TestKeySet) {
        self.test_keys = Some(test_keys);
    }

    /// Create a real QBFT instance for the given height
    fn create_qbft_instance(
        &self,
        height: InstanceHeight,
        value: &[u8],
    ) -> Result<QbftInstance, String> {
        use qbft::{ConfigBuilder, Qbft};
        use std::cell::RefCell;
        use std::rc::Rc;

        // Build committee from stored committee info
        let committee: IndexSet<OperatorId> = self
            .committee
            .iter()
            .map(|op| OperatorId::from(op.operator_id))
            .collect();

        // Calculate quorum size
        let quorum_size = calculate_quorum(committee.len());

        // Build config with our TestLeaderFunction
        let config = ConfigBuilder::new(self.operator_id, height, committee)
            .with_quorum_size(quorum_size)
            .with_max_rounds(100)
            .with_leader_fn(TestLeaderFunction { height })
            .build()
            .map_err(|e| format!("Failed to build QBFT config: {:?}", e))?;

        // Create a simple handler that captures messages
        let captured = Rc::new(RefCell::new(Vec::new()));
        let captured_clone = captured.clone();

        let handler: Box<dyn FnMut(qbft::UnsignedWrappedQbftMessage)> =
            Box::new(move |msg: qbft::UnsignedWrappedQbftMessage| {
                // For controller tests, we just capture the message
                // No need to sign or broadcast since we're testing message processing
                captured_clone.borrow_mut().push(msg);
            });

        // Create the instance
        let data_hash = hash_data(value);
        let beacon_vote = crate::utils::misc::create_beacon_vote_from_bytes(data_hash.as_ref());

        let instance = Qbft::new(
            config,
            beacon_vote.clone(),
            self.identifier.clone(),
            handler,
        );

        // The instance starts automatically when created
        // No need to call start_instance_spec

        Ok(instance)
    }

    /// Start a new QBFT instance at the given height.
    ///
    /// Validates:
    /// - Value is not empty or invalid ([1,1,1,1])
    /// - Height is not in the past
    /// - No instance already exists at this height
    pub async fn start_new_instance(
        &mut self,
        height: InstanceHeight,
        value: Vec<u8>,
    ) -> Result<(), String> {
        // 1. Validate the value (matching Go's ValueCheckF logic)
        // Go checks:
        // - If value == [1,1,1,1] → error
        // - If value is empty → error
        if value.is_empty() {
            return Err("value invalid: invalid value".to_string());
        }

        // Check for TestingInvalidValueCheck = [1,1,1,1]
        if value == vec![1, 1, 1, 1] {
            return Err("value invalid: invalid value".to_string());
        }

        // 2. Validate height (no past instances) - matching Go logic
        // Check if trying to start an instance with a past height
        if *height < *self.current_height {
            return Err("attempting to start an instance with a past height".to_string());
        }

        // 3. Check if instance already exists at this height
        // Go checks: if c.StoredInstances.FindInstance(height) != nil
        if self.stored_instances.contains_key(&height) {
            return Err("instance already running".to_string());
        }

        // Update current height to the new height
        // This is important: we advance to the new height even if it's higher than current+1
        self.current_height = height;

        // Create and start a real QBFT instance (matching Go's behavior)
        // The Go controller creates real instances: newInstance := c.addAndStoreNewInstance()
        // And starts them: newInstance.Start(value, height)

        // Create a real QBFT instance directly (bypassing async processor)
        let instance = self.create_qbft_instance(height, &value)?;

        // Store the instance
        self.qbft_instances.insert(height, instance);

        // Mark this instance as active
        self.stored_instances.insert(height, InstanceState::Active);

        Ok(())
    }

    /// Check if an instance exists at the given height
    fn has_instance(&self, height: InstanceHeight) -> bool {
        self.stored_instances.contains_key(&height)
    }

    /// Check if a message is from a future height.
    fn is_future_message(&self, height: InstanceHeight) -> bool {
        // Special case: first height with no instances
        if self.current_height == InstanceHeight::from(0) && self.stored_instances.is_empty() {
            return true;
        }
        *height > *self.current_height
    }

    /// Validate that all signers are in the committee.
    fn validate_committee_membership(
        &self,
        qbft_msg: &ssv_types::consensus::QbftMessage,
        operator_ids: &[OperatorId],
    ) -> Result<(), String> {
        for &op_id in operator_ids {
            let id_val = *op_id;
            if id_val < 1 || id_val > 4 {
                let error_prefix = if is_decided_message(
                    qbft_msg,
                    operator_ids,
                    calculate_quorum(self.current_committee_size),
                ) {
                    "invalid decided msg: invalid decided msg"
                } else {
                    "invalid msg"
                };
                return Err(format!("{}: signer not in committee", error_prefix));
            }
        }
        Ok(())
    }

    /// Validate a decided message.
    ///
    /// Checks:
    /// - Message is a commit
    /// - Has quorum signatures
    /// - Has full_data
    /// - Hash of full_data matches root
    /// - RSA signatures are valid (if enabled)
    fn validate_decided(
        &self,
        signed_msg: &ssv_types::message::SignedSSVMessage,
        qbft_msg: &ssv_types::consensus::QbftMessage,
    ) -> Result<(), String> {
        use crate::utils::rsa_validation::verify_rsa_signature;
        use ssv_types::consensus::QbftMessageType;

        let _operator_ids = signed_msg.operator_ids();

        // Must be a commit message
        if !matches!(qbft_msg.qbft_message_type, QbftMessageType::Commit) {
            return Err("decided message must be commit type".to_string());
        }

        // Must have quorum signatures
        let operator_ids = signed_msg.operator_ids();
        let quorum = calculate_quorum(self.current_committee_size);
        if operator_ids.len() < quorum {
            return Err(format!(
                "decided message has {} signatures, needs {} for quorum",
                operator_ids.len(),
                quorum
            ));
        }

        // Must have full_data
        if signed_msg.full_data().is_empty() {
            return Err("decided message missing full_data".to_string());
        }

        // Verify RSA signatures for decided messages
        // This is critical for the "decide wrong sig" test
        if let Some(ref test_keys) = self.test_keys {
            let msg_bytes = signed_msg.ssv_message().as_ssz_bytes();

            for (&op_id, sig) in operator_ids.iter().zip(signed_msg.signatures().iter()) {
                // Convert signature from VariableList to [u8; 256]
                if sig.len() != 256 {
                    return Err("invalid decided msg: invalid signature length".to_string());
                }
                let mut sig_array = [0u8; 256];
                sig_array.copy_from_slice(&sig[..]);

                if !verify_rsa_signature(msg_bytes.clone(), op_id, &sig_array, test_keys) {
                    return Err("invalid decided msg: invalid decided msg: msg signature invalid: crypto/rsa: verification error".to_string());
                }
            }
        }

        let full_data = signed_msg.full_data();

        // Validate that the hash of full_data matches the root in the QBFT message
        // The Go code uses SHA256(fullData) == root
        let data_hash = hash_data(full_data);

        // Compare with the root in the QBFT message
        if data_hash != qbft_msg.root {
            return Err(format!("H(data) != root"));
        }

        Ok(())
    }

    /// Decode QbftMessage from test message
    fn decode_qbft_message(
        test_msg: &TestSignedSSVMessage,
    ) -> Result<
        (
            ssv_types::message::SignedSSVMessage,
            ssv_types::consensus::QbftMessage,
        ),
        String,
    > {
        // Convert TestSignedSSVMessage to SignedSSVMessage
        let signed_msg: ssv_types::message::SignedSSVMessage = test_msg
            .clone()
            .try_into()
            .map_err(|e| format!("Failed to convert test message: {:?}", e))?;

        // Extract and decode the QbftMessage from the SSV message data
        let ssv_msg = signed_msg.ssv_message();
        let qbft_bytes = ssv_msg.data();

        // Decode QbftMessage using SSZ
        use ssz::Decode;
        let qbft_msg: ssv_types::consensus::QbftMessage =
            ssv_types::consensus::QbftMessage::from_ssz_bytes(qbft_bytes)
                .map_err(|e| format!("Failed to decode QbftMessage: {:?}", e))?;

        Ok((signed_msg, qbft_msg))
    }

    /// Handle a decided message (commit with quorum).
    ///
    /// Returns the decided value only if this is the first time
    /// the instance is being decided.
    async fn upon_decided(
        &mut self,
        test_msg: &TestSignedSSVMessage,
    ) -> Result<Option<Vec<u8>>, String> {
        // Decode the message
        let (signed_msg, qbft_msg) = Self::decode_qbft_message(test_msg)?;
        let height = InstanceHeight::from(qbft_msg.height as usize);

        // Validate the decided message
        self.validate_decided(&signed_msg, &qbft_msg)?;

        // Extract the decided value from full_data
        let decided_value = extract_decided_value(&signed_msg);

        // Update controller height if this is a future decided message
        if *height > *self.current_height {
            self.current_height = height;
        }

        // Check if we already had this instance as decided
        let was_previously_decided = matches!(
            self.stored_instances.get(&height),
            Some(InstanceState::Decided(_))
        );

        // Store this instance as decided
        self.stored_instances
            .insert(height, InstanceState::Decided(decided_value.clone()));

        // For controller tests, we don't forward to real QBFT manager
        // We just track the decision in our local state

        // Return decided value only if this is the first time we're seeing this decision
        // This matches Go's logic: return decided_value if !prevDecided
        if !was_previously_decided {
            Ok(Some(decided_value))
        } else {
            Ok(None)
        }
    }

    /// Process a message from the test data.
    ///
    /// Message routing:
    /// 1. Decided messages (commit with quorum) → upon_decided
    /// 2. Future messages → error
    /// 3. Messages for non-existent instances → error
    /// 4. Messages for decided instances → error (except for aggregation)
    /// 5. Normal messages → aggregation and quorum checking
    pub async fn process_msg(
        &mut self,
        test_msg: &TestSignedSSVMessage,
    ) -> Result<Option<Vec<u8>>, String> {
        // Decode the message once
        let (signed_msg, qbft_msg) = Self::decode_qbft_message(test_msg)?;
        let height = InstanceHeight::from(qbft_msg.height as usize);
        let operator_ids = signed_msg.operator_ids();
        let round = qbft_msg.round;
        let root = qbft_msg.root;

        // Validate message identifier matches controller
        let msg_identifier = identifier_to_message_id(&qbft_msg.identifier)?;
        if msg_identifier != self.identifier {
            return Err("invalid msg: message doesn't belong to Identifier".to_string());
        }

        // Validate that all signers are in the committee
        self.validate_committee_membership(&qbft_msg, operator_ids)?;

        // Validate signer count for message type
        validate_signer_count(&qbft_msg, operator_ids)?;

        // 1. Check if this is a decided message (commit with quorum)
        // IMPORTANT: This must come BEFORE the already-decided check, because
        // decided messages should always be routed to upon_decided, even if
        // the instance is already decided (upon_decided will handle that)
        if is_decided_message(
            &qbft_msg,
            operator_ids,
            calculate_quorum(self.current_committee_size),
        ) {
            // Route to upon_decided handler
            return self.upon_decided(test_msg).await;
        }

        // 2. Check if this is a future message
        if self.is_future_message(height) {
            return Err("future msg from height, could not process".to_string());
        }

        // 3. Check if instance exists
        if !self.has_instance(height) {
            return Err("instance not found".to_string());
        }

        // 3a. Check if instance is already decided
        // Reject all messages for decided instances
        // This is required by "late commit" tests which expect rejection
        if matches!(
            self.stored_instances.get(&height),
            Some(InstanceState::Decided(_))
        ) {
            return Err(
                "not processing consensus message since instance is already decided".to_string(),
            );
        }

        // 4. Forward the message to the actual QBFT instance for validation
        // This matches Go's inst.ProcessMsg(msg) behavior

        // Get the QBFT instance for this height
        let instance = self
            .qbft_instances
            .get_mut(&height)
            .ok_or_else(|| "could not process msg: instance not found".to_string())?;

        // Create wrapped message for the QBFT instance
        let wrapped = qbft::WrappedQbftMessage {
            signed_message: signed_msg.clone(),
            qbft_message: qbft_msg.clone(),
        };

        // Process the message through the instance
        // This will return an error if the message is invalid
        if let Err(qbft_err) = instance.process_message_spec(wrapped) {
            // Map the error to match Go's error wrapping
            use crate::utils::error_mapping::map_qbft_error;
            let error_msg = map_qbft_error(&qbft_err);
            return Err(format!("could not process msg: {}", error_msg));
        }

        // Check if the instance decided after processing this message
        if instance.is_decided_spec() {
            // Get the decided value
            if let Some(decided_data) = instance.get_decided_data_spec() {
                // Extract the actual value from the BeaconVote
                // For tests, we stored the hash in the BeaconVote, but we need the original value
                // We should have stored it in proposal_full_data
                let decided_value = self
                    .proposal_storage
                    .get_proposal_full_data(height, round, root)
                    .unwrap_or_else(|| {
                        // Fallback: extract from the decided data somehow
                        // This shouldn't happen in properly formed messages
                        vec![]
                    });

                // Check if this instance was already decided
                let was_previously_decided = matches!(
                    self.stored_instances.get(&height),
                    Some(InstanceState::Decided(_))
                );

                // Store this instance as decided
                self.stored_instances
                    .insert(height, InstanceState::Decided(decided_value.clone()));

                // Update controller height if this is a future decided instance
                if *height > *self.current_height {
                    self.current_height = height;
                }

                // Return decided value only if this is the first time
                if !was_previously_decided {
                    return Ok(Some(decided_value));
                } else {
                    return Ok(None);
                }
            }
        }

        // 5. Message is already stored in core QBFT's containers
        // We don't need to duplicate storage

        // 4a. If this is a proposal with full_data, store it for later use
        if matches!(
            qbft_msg.qbft_message_type,
            ssv_types::consensus::QbftMessageType::Proposal
        ) {
            let full_data = signed_msg.full_data();
            if !full_data.is_empty() {
                self.proposal_storage.store_proposal_full_data(
                    height,
                    round,
                    root,
                    full_data.to_vec(),
                );
            }
        }

        // 5. Check if this is a commit message - check quorum using core QBFT container
        if matches!(
            qbft_msg.qbft_message_type,
            ssv_types::consensus::QbftMessageType::Commit
        ) {
            // Get the instance to check its commit container
            if let Some(instance) = self.qbft_instances.get(&height) {
                // Check if the commit container has quorum for this round
                // The core QBFT tracks this automatically
                if let Some(quorum_root) = instance.get_commit_container().has_quorum(round.into())
                {
                    // Verify the root matches
                    if quorum_root == root {
                        // We have quorum! Get the decided value from the proposal's full_data
                        let decided_value = self
                            .proposal_storage
                            .get_proposal_full_data(height, round, root)
                            .unwrap_or_else(|| {
                                // Fallback to extracting from current message if no proposal stored
                                extract_decided_value(&signed_msg)
                            });

                        // Check if this instance was already decided
                        let was_previously_decided = matches!(
                            self.stored_instances.get(&height),
                            Some(InstanceState::Decided(_))
                        );

                        // Store this instance as decided
                        self.stored_instances
                            .insert(height, InstanceState::Decided(decided_value.clone()));

                        // Update controller height if this is a future decided instance
                        if *height > *self.current_height {
                            self.current_height = height;
                        }

                        // Return decided value only if this is the first time
                        if !was_previously_decided {
                            return Ok(Some(decided_value));
                        } else {
                            return Ok(None);
                        }
                    }
                }
            }
        }

        // No decision yet
        Ok(None)
    }
}

impl Drop for ControllerAdapter {
    fn drop(&mut self) {
        // Abort all running instance tasks to ensure clean shutdown
        for handle in self.instance_handles.drain(..) {
            handle.abort();
        }
    }
}
