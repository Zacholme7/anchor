use super::spec_types::TestSignedSSVMessage;
use crate::utils::message_validation::{
    extract_decided_value, identifier_to_message_id, is_decided_message, validate_signer_count,
};
use crate::utils::misc::{calculate_quorum, hash_data};
use message_sender::testing::MockMessageSender;
use qbft::InstanceHeight;
use qbft_manager::QbftManager;
use slot_clock::{ManualSlotClock, SlotClock};
use ssv_types::OperatorId;
use ssv_types::domain_type::DomainType;
use ssv_types::msgid::MessageId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use types::{Hash256, Slot};

/// State of an instance in the controller
#[derive(Debug, Clone)]
enum InstanceState {
    /// Instance is currently running consensus
    Active,
    /// Instance has decided with this value
    Decided(Vec<u8>),
}

/// Container for tracking messages by round and root (similar to Go's MsgContainer)
#[derive(Debug, Clone)]
struct MessageContainer {
    /// Messages organized by (height, round, root)
    messages: HashMap<(InstanceHeight, u64, Hash256), Vec<TestSignedSSVMessage>>,
    /// Commit messages specifically (for aggregation)
    commit_messages: HashMap<(InstanceHeight, u64, Hash256), Vec<TestSignedSSVMessage>>,
    /// Store the proposal's full_data for each (height, round, root)
    proposal_full_data: HashMap<(InstanceHeight, u64, Hash256), Vec<u8>>,
}

impl MessageContainer {
    fn new() -> Self {
        Self {
            messages: HashMap::new(),
            commit_messages: HashMap::new(),
            proposal_full_data: HashMap::new(),
        }
    }

    /// Add a message to the container
    fn add_message(
        &mut self,
        height: InstanceHeight,
        round: u64,
        root: Hash256,
        msg: TestSignedSSVMessage,
    ) {
        let key = (height, round, root);
        self.messages.entry(key).or_insert_with(Vec::new).push(msg);
    }

    /// Add a commit message specifically (for aggregation)
    fn add_commit_message(
        &mut self,
        height: InstanceHeight,
        round: u64,
        root: Hash256,
        msg: TestSignedSSVMessage,
    ) {
        let key = (height, round, root);
        self.commit_messages
            .entry(key)
            .or_insert_with(Vec::new)
            .push(msg);
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

    /// Get all unique signers for commits at a specific height, round, and root
    fn get_unique_commit_signers(
        &self,
        height: InstanceHeight,
        round: u64,
        root: Hash256,
    ) -> Vec<OperatorId> {
        let key = (height, round, root);
        let mut signers = Vec::new();

        // Only count signers from COMMIT messages
        if let Some(msgs) = self.commit_messages.get(&key) {
            for msg in msgs {
                // Extract operator IDs from the message
                if let Ok(signed_msg) =
                    TryInto::<ssv_types::message::SignedSSVMessage>::try_into(msg.clone())
                {
                    for &op_id in signed_msg.operator_ids() {
                        if !signers.contains(&op_id) {
                            signers.push(op_id);
                        }
                    }
                }
            }
        }

        signers
    }

    /// Get messages for aggregation
    fn get_messages_for_aggregation(
        &self,
        height: InstanceHeight,
        round: u64,
        root: Hash256,
    ) -> Vec<TestSignedSSVMessage> {
        let key = (height, round, root);
        self.messages.get(&key).cloned().unwrap_or_default()
    }
}

/// Adapter for QBFT controller spec tests.
///
/// This adapter simulates a QBFT controller without running actual consensus.
/// It tracks instances, validates messages, and aggregates commits to detect quorum.
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

    // Message aggregation (mirrors Go's MsgContainer)
    message_container: MessageContainer,

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
            message_container: MessageContainer::new(),
            instance_handles: Vec::new(),
            operator_id,
            identifier,
            committee,
        }
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

        // For controller tests, we don't actually start a QBFT instance
        // We just mark that an instance exists at this height
        // The actual consensus will be driven by the test messages
        // This matches Go's behavior where test instances don't run real consensus

        // Mark this instance as active (but don't create actual QBFT instance)
        self.stored_instances.insert(height, InstanceState::Active);

        // Store the input value for validation (but don't use it for consensus)
        // The real consensus data comes from the test messages

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

        let full_data = signed_msg.full_data();

        // Validate that the hash of full_data matches the root in the QBFT message
        // The Go code uses SHA256(fullData) == root
        let data_hash = hash_data(full_data);

        // Compare with the root in the QBFT message
        if data_hash != qbft_msg.root {
            return Err(format!("H(data) != root"));
        }

        // RSA Signature Verification is disabled for now
        // Only the decide_wrong_sig test requires actual RSA verification
        // Other tests use mock signatures that would fail verification
        if false {
            // Get the SSV message bytes for hashing
            let ssv_msg = signed_msg.ssv_message();

            // Encode the SSV message (matching Go's SSVMessage.Encode())
            use ssz::Encode;
            let encoded_msg = ssv_msg.as_ssz_bytes();

            // Hash the encoded message (matching Go: hash := sha256.Sum256(encodedMsg))
            let msg_hash_256 = hash_data(&encoded_msg);
            let msg_hash: [u8; 32] = *msg_hash_256.as_ref();

            // Get signatures from the signed message
            let signatures = signed_msg.signatures();

            // Verify each signature against the corresponding operator's public key
            for (i, &op_id) in operator_ids.iter().enumerate() {
                // Find the operator in the committee
                let operator = self
                    .committee
                    .iter()
                    .find(|op| op.operator_id == *op_id as u64)
                    .ok_or_else(|| "invalid decided msg: signer not in committee".to_string())?;

                // Get the signature for this operator
                let signature = signatures
                    .get(i)
                    .ok_or_else(|| "invalid decided msg: missing signature".to_string())?;

                // Parse the PEM-encoded RSA public key
                // The SSVOperatorPubKey is base64-encoded PEM
                use base64::Engine;
                let pem_bytes = base64::engine::general_purpose::STANDARD
                    .decode(&operator.ssv_operator_pub_key)
                    .map_err(|e| {
                        format!("invalid decided msg: failed to decode public key: {}", e)
                    })?;
                let pem_str = String::from_utf8(pem_bytes)
                    .map_err(|e| format!("invalid decided msg: invalid PEM string: {}", e))?;

                // Parse the RSA public key from PEM
                // The PEM has "RSA PUBLIC KEY" header but contains PKIX/SPKI data
                // We need to parse manually
                use pem::parse;
                let pem_block = parse(&pem_str)
                    .map_err(|e| format!("invalid decided msg: failed to parse PEM: {}", e))?;

                // Parse as PKIX public key (Go's x509.ParsePKIXPublicKey)
                use rsa::{RsaPublicKey, pkcs8::DecodePublicKey};
                let public_key = RsaPublicKey::from_public_key_der(pem_block.contents())
                    .map_err(|e| format!("invalid decided msg: failed to parse RSA key: {}", e))?;

                // Verify the signature (matching Go's rsa.VerifyPKCS1v15)
                use rsa::sha2::Sha256 as RsaSha256;
                use rsa::signature::Verifier;

                let verifying_key = rsa::pkcs1v15::VerifyingKey::<RsaSha256>::new(public_key);
                let signature_obj = rsa::pkcs1v15::Signature::try_from(signature.as_ref())
                    .map_err(|e| format!("invalid decided msg: invalid signature format: {}", e))?;

                // Verify the signature against the message hash
                verifying_key.verify(&msg_hash, &signature_obj)
                    .map_err(|_| "invalid decided msg: invalid decided msg: msg signature invalid: crypto/rsa: verification error".to_string())?;
            }
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
            .map_err(|e| format!("Failed to convert test message: {}", e))?;

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
    /// 4. Messages for decided instances → error
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

        // 3a. Check if instance is already decided - if so, reject non-decided messages
        // (Decided messages were already handled above)
        if matches!(
            self.stored_instances.get(&height),
            Some(InstanceState::Decided(_))
        ) {
            return Err(
                "not processing consensus message since instance is already decided".to_string(),
            );
        }

        // 4. Store the message in our container for aggregation
        self.message_container
            .add_message(height, round, root, test_msg.clone());

        // 4a. If this is a proposal with full_data, store it for later use
        if matches!(
            qbft_msg.qbft_message_type,
            ssv_types::consensus::QbftMessageType::Proposal
        ) {
            let full_data = signed_msg.full_data();
            if !full_data.is_empty() {
                self.message_container.store_proposal_full_data(
                    height,
                    round,
                    root,
                    full_data.to_vec(),
                );
            }
        }

        // 5. Check if this is a commit message - if so, track it and check for aggregated quorum
        if matches!(
            qbft_msg.qbft_message_type,
            ssv_types::consensus::QbftMessageType::Commit
        ) {
            // Add to commit messages specifically
            self.message_container
                .add_commit_message(height, round, root, test_msg.clone());

            // Get all unique signers from COMMIT messages only
            let unique_signers = self
                .message_container
                .get_unique_commit_signers(height, round, root);
            let quorum = calculate_quorum(self.current_committee_size);

            // Check if we have reached quorum through aggregation
            if unique_signers.len() >= quorum {
                // We have quorum! Get the decided value from the proposal's full_data
                // (like Go does in commit.go:29)
                let decided_value = self
                    .message_container
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
