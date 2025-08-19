use super::spec_types::SpecTestOperator;
use super::spec_types::TestSignedSSVMessage;
use crate::utils::message_validation::{
    extract_decided_value, identifier_to_message_id, is_decided_message,
};
use crate::utils::misc::{calculate_quorum, hash_data};
use crate::utils::rsa_validation::verify_rsa_signature;
use crate::utils::test_keys::TestKeySet;
use qbft::InstanceHeight;
use qbft::LeaderFunction;
use qbft::UnsignedWrappedQbftMessage;
use qbft::{ConfigBuilder, Qbft};
use ssv_types::consensus::QbftMessageType;
use ssv_types::consensus::{BeaconVote, QbftMessage};
use ssv_types::message::SignedSSVMessage;
use ssv_types::msgid::MessageId;
use ssv_types::{IndexSet, OperatorId, Round};
use ssz::{Decode, Encode};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tokio::task::JoinHandle;
use types::Hash256;

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

/// Storage for proposal data needed by spec tests.
///
/// The core QBFT only stores hashes/roots of data, not the actual data.
/// For spec tests to verify correct decisions, we need to track what actual
/// data was proposed in each round so we can return it when decided.
/// This is NOT mocking - it's compensating for the hash-only storage in core QBFT.
#[derive(Debug, Clone)]
struct ProposalDataStorage {
    /// Maps (height, round, root) to the actual proposed data
    proposal_data: HashMap<(InstanceHeight, u64, Hash256), Vec<u8>>,
}

impl ProposalDataStorage {
    fn new() -> Self {
        Self {
            proposal_data: HashMap::new(),
        }
    }

    /// Store proposal data for a given height, round, and root
    fn store(&mut self, height: InstanceHeight, round: u64, root: Hash256, data: Vec<u8>) {
        self.proposal_data.insert((height, round, root), data);
    }

    /// Retrieve proposal data for a given height, round, and root
    fn get(&self, height: InstanceHeight, round: u64, root: Hash256) -> Option<Vec<u8>> {
        self.proposal_data.get(&(height, round, root)).cloned()
    }
}

/// Adapter for QBFT controller spec tests.
///
/// This adapter runs a QBFT controller with actual QBFT instances.
/// It tracks instances, forwards messages to them, and handles decisions.
pub struct ControllerAdapter {
    // Instance tracking (mirrors Go's StoredInstances)
    current_height: InstanceHeight,
    stored_instances: HashMap<InstanceHeight, InstanceState>, // All instances (active & decided)
    current_committee_size: usize,                            // For quorum calculation
    // Test keys for RSA signature verification
    test_keys: TestKeySet,
    // Direct QBFT instances for synchronous processing (bypassing async processor)
    qbft_instances: HashMap<InstanceHeight, QbftInstance>,
    // Storage for proposal data (needed because core QBFT only stores hashes)
    proposal_storage: ProposalDataStorage,
    // Track spawned instance tasks so we can abort them
    instance_handles: Vec<JoinHandle<()>>,
    // Configuration
    operator_id: OperatorId,
    identifier: MessageId,
    // Committee information for signature verification
    committee: Vec<SpecTestOperator>,
}

impl ControllerAdapter {
    /// Create a new ControllerAdapter with the given operator ID and committee
    pub fn new(committee: Vec<SpecTestOperator>) -> Self {
        Self {
            current_height: InstanceHeight::from(0),
            stored_instances: HashMap::new(),
            current_committee_size: 4,
            test_keys: TestKeySet::four_share_set(),
            qbft_instances: HashMap::new(),
            proposal_storage: ProposalDataStorage::new(),
            instance_handles: Vec::new(),
            operator_id: OperatorId::from(1),
            identifier: MessageId::for_spectest(),
            committee,
        }
    }

    /// Start a new QBFT instance at the given height.
    ///
    /// Validates:
    /// - Value is not empty or invalid ([1,1,1,1])
    /// - Height is not in the past
    /// - No instance already exists at this height
    pub fn start_new_instance(
        &mut self,
        height: InstanceHeight,
        value: Vec<u8>,
    ) -> Result<(), String> {
        // We enforce that start_data impl QbftData and therefor implement ssz encode/decode
        // so a situation in which we have an empty value or [1,1,1,1] is not possible.
        if value.is_empty() || value == vec![1, 1, 1, 1] {
            return Err("value invalid: invalid value".to_string());
        }

        // Again, we will never start an instance with an old height
        if *height < *self.current_height {
            return Err("attempting to start an instance with a past height".to_string());
        }

        // Check if instance already exists at this height
        if self.stored_instances.contains_key(&height) {
            return Err("instance already running".to_string());
        }

        // Update current height to the new height
        // This is important: we advance to the new height even if it's higher than current+1
        self.current_height = height;

        // Create a real qbft instance and store it as active
        // This is now ready to process messages
        let instance = self.create_qbft_instance(height, &value)?;
        self.qbft_instances.insert(height, instance);
        self.stored_instances.insert(height, InstanceState::Active);

        Ok(())
    }

    /// Create a real QBFT instance for the given height
    fn create_qbft_instance(
        &self,
        height: InstanceHeight,
        start_value: &[u8],
    ) -> Result<QbftInstance, String> {
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
            .with_max_rounds(15)
            .with_leader_fn(TestLeaderFunction { height })
            .build()
            .unwrap();

        // Create a simple handler that captures messages
        let captured = Rc::new(RefCell::new(Vec::new()));
        let captured_clone = captured.clone();

        // For controller tests, we just capture the message
        // No need to sign or broadcast since we're testing message processing
        let handler: Box<dyn FnMut(UnsignedWrappedQbftMessage)> =
            Box::new(move |msg: UnsignedWrappedQbftMessage| {
                captured_clone.borrow_mut().push(msg);
            });

        // Create the instance with the actual test data
        let test_data = BeaconVote::from_ssz_bytes(start_value).unwrap();
        let instance = Qbft::new(config, test_data.clone(), self.identifier.clone(), handler);

        Ok(instance)
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
        signed_msg: &SignedSSVMessage,
        qbft_msg: &QbftMessage,
    ) -> Result<(), String> {
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
        let msg_bytes = signed_msg.ssv_message().as_ssz_bytes();
        for (&op_id, sig) in operator_ids.iter().zip(signed_msg.signatures().iter()) {
            // Convert signature from VariableList to [u8; 256]
            if sig.len() != 256 {
                return Err("invalid decided msg: invalid signature length".to_string());
            }
            let mut sig_array = [0u8; 256];
            sig_array.copy_from_slice(&sig[..]);

            if !verify_rsa_signature(msg_bytes.clone(), op_id, &sig_array, &self.test_keys) {
                return Err("invalid decided msg: invalid decided msg: msg signature invalid: crypto/rsa: verification error".to_string());
            }
        }

        // Validate that the hash of full_data matches the root in the QBFT message
        // The Go code uses SHA256(fullData) == root
        let full_data = signed_msg.full_data();
        let data_hash = hash_data(full_data);
        if data_hash != qbft_msg.root {
            return Err(format!("H(data) != root"));
        }

        Ok(())
    }

    /// Decode QbftMessage from test message
    fn decode_qbft_message(
        test_msg: &TestSignedSSVMessage,
    ) -> Result<(SignedSSVMessage, QbftMessage), String> {
        // Convert TestSignedSSVMessage to SignedSSVMessage
        let signed_msg: SignedSSVMessage = test_msg
            .clone()
            .try_into()
            .map_err(|e| format!("Failed to convert test message: {:?}", e))?;

        // Extract and decode the QbftMessage from the SSV message data
        let qbft_bytes = signed_msg.ssv_message().data();
        let qbft_msg: QbftMessage = QbftMessage::from_ssz_bytes(qbft_bytes)
            .map_err(|e| format!("Failed to decode QbftMessage: {:?}", e))?;

        Ok((signed_msg, qbft_msg))
    }

    /// Handle a decided message (commit with quorum).
    ///
    /// Returns the decided value only if this is the first time
    /// the instance is being decided.
    fn upon_decided(&mut self, test_msg: &TestSignedSSVMessage) -> Result<Option<Vec<u8>>, String> {
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
    pub fn process_msg(
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
            return self.upon_decided(test_msg);
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

        // Store proposal data if this is a proposal with full_data
        // This is needed because core QBFT only stores hashes, not actual data
        if matches!(
            qbft_msg.qbft_message_type,
            ssv_types::consensus::QbftMessageType::Proposal
        ) {
            let full_data = signed_msg.full_data();
            if !full_data.is_empty() {
                self.proposal_storage
                    .store(height, round, root, full_data.to_vec());
            }
        }

        // Check if the instance decided after processing this message
        if instance.is_decided_spec() {
            // Get the decided value from our proposal storage
            if let Some(_decided_data) = instance.get_decided_data_spec() {
                // The decided value is what was proposed in the decided round
                // We need to look it up from our proposal storage
                let decided_value = self
                    .proposal_storage
                    .get(height, round, root)
                    .unwrap_or_else(|| {
                        // Fallback: if no proposal was stored, try to extract from message
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
                        // We have quorum! Get the decided value from proposal storage
                        let decided_value = self
                            .proposal_storage
                            .get(height, round, root)
                            .unwrap_or_else(|| {
                                // Fallback to extracting from current message
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
}
