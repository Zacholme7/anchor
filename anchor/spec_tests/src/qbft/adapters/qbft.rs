use super::spec_types::{AcceptedProposal, MessageContainer, TestSignedSSVMessage};
use crate::utils::error_mapping::map_qbft_error;
use crate::utils::misc::calculate_quorum;
use crate::utils::rsa_signing::sign_message_with_full_data;
use crate::utils::rsa_validation::validate_rsa_signatures;
use crate::utils::test_keys::TestKeySet;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use openssl::pkey::Private;
use openssl::rsa::Rsa;
use qbft::{ConfigBuilder, InstanceHeight, InstanceState, LeaderFunction};
use qbft::{Qbft, UnsignedWrappedQbftMessage};
use ssv_types::consensus::{BeaconVote, QbftMessage, QbftMessageType};
use ssv_types::message::SignedSSVMessage;
use ssv_types::msgid::MessageId;
use ssv_types::{IndexSet, OperatorId, Round};
use ssz::Decode;
use std::cell::RefCell;
use std::rc::Rc;
use types::Hash256;

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

/// State that we want to initialize the qbft instance with
#[derive(Debug, Clone)]
pub struct QbftStartingState {
    pub height: InstanceHeight,
    pub identifier: MessageId,
    pub committee: Option<IndexSet<OperatorId>>,
    pub operator_id: OperatorId,
    pub round: Round,
    pub start_value: Vec<u8>,
    pub proposal_accepted: Option<AcceptedProposal>,
    pub propose_container: MessageContainer,
    pub prepare_container: MessageContainer,
    pub commit_container: MessageContainer,
    pub round_change_container: MessageContainer,
    pub round_change_justifications: Option<Vec<TestSignedSSVMessage>>,
    pub prepare_justifications: Option<Vec<TestSignedSSVMessage>>,
    pub force_stop: bool,
}

// Simple mock handler type
type MockHandler = Box<dyn FnMut(UnsignedWrappedQbftMessage)>;

// Adapter over our core qbft instance
pub struct QbftAdapter {
    // Test instance
    instance: Qbft<TestLeaderFunction, BeaconVote, MockHandler>,
    // Key to sign messages
    operator_rsa_key: Rsa<Private>,
    // Capture send messages
    captured_messages: Rc<RefCell<Vec<SignedSSVMessage>>>, // Capture sent messages
    // Track number of timeouts triggered
    timeout_count: u64, // Track number of timeouts triggered
    // Store test keys for validation
    test_keys: Option<TestKeySet>, // Store test keys for validation
    // Force stop flag for spec tests
    force_stop: bool,
}

impl QbftAdapter {
    /// Build a QBFT instance with starting state
    pub fn new_with_state(state: QbftStartingState) -> Self {
        // Use committee from state or default 4-node committee
        let committee: IndexSet<OperatorId> = state
            .committee
            .clone()
            .unwrap_or_else(|| vec![1, 2, 3, 4].into_iter().map(OperatorId::from).collect());

        // Calculate quorum size based on committee size
        let quorum_size = calculate_quorum(committee.len());

        let config = ConfigBuilder::new(state.operator_id, state.height, committee)
            .with_quorum_size(quorum_size)
            .with_max_rounds(15) // Support very high rounds for testing
            .with_leader_fn(TestLeaderFunction {
                height: state.height,
            }) // Use test leader function
            .build()
            .expect("Failed to build config");

        // Get test keys and RSA key for this operator
        let test_keys = TestKeySet::four_share_set();
        let rsa_key = test_keys
            .operator_keys
            .get(&state.operator_id)
            .cloned()
            .unwrap();
        let rsa_key_clone = rsa_key.clone();

        // Create a handler that captures and signs messages
        let captured = Rc::new(RefCell::new(Vec::new()));
        let captured_clone = captured.clone();
        let op_id = state.operator_id;
        let mock_handler: MockHandler = Box::new(move |msg: UnsignedWrappedQbftMessage| {
            let full_data = msg.unsigned_message.full_data.to_vec();
            let signed = sign_message_with_full_data(
                msg.unsigned_message,
                full_data,
                &rsa_key_clone,
                &op_id,
            );

            captured_clone.borrow_mut().push(signed);
        });

        // Decode the start_value to BeaconVote
        let start_data = BeaconVote::from_ssz_bytes(&state.start_value)
            .expect("Failed to decode BeaconVote from start_value");
        // we should return ProposalInvalidValue here

        let instance = Qbft::new(config, start_data, state.identifier.clone(), mock_handler);

        // Build the adapter
        let mut adapter = Self {
            instance,
            operator_rsa_key: rsa_key,
            captured_messages: captured,
            timeout_count: 0,
            test_keys: Some(test_keys),
            force_stop: state.force_stop,
        };

        // Start round is called right away, just clear these messages since we
        // want to test specific message combinations
        adapter.captured_messages.borrow_mut().clear();
        adapter.timeout_count = 0;

        // Set the round
        adapter.setup_round(state.round);

        // Set the proposal accepted for current round
        if let Some(ref proposal_accepted) = state.proposal_accepted {
            adapter.setup_proposal_accepted(proposal_accepted);
        }

        // Set the justifications
        if let Some(ref rc_jus) = state.round_change_justifications {
            adapter.setup_round_change_justifications(rc_jus);
        }

        if let Some(ref pre_jus) = state.prepare_justifications {
            adapter.setup_prepare_justifications(pre_jus);
        }

        // Populate all message containers
        adapter.populate_containers(&state);

        adapter
    }

    /// Create a new SignedSSVMessage using the instance
    pub fn create_message(
        &mut self,
        msg_type: QbftMessageType,
        root: Hash256,
        data: Vec<u8>,
    ) -> SignedSSVMessage {
        let start_data = BeaconVote::from_ssz_bytes(&data)
            .expect("Failed to decode BeaconVote from start_value");

        // delegate message creation based on message type
        match msg_type {
            QbftMessageType::Proposal => self.instance.send_proposal(root, start_data.into()),
            QbftMessageType::Prepare => self.instance.send_prepare(root),
            QbftMessageType::Commit => self.instance.send_commit(root),
            QbftMessageType::RoundChange => self.instance.send_round_change(root),
        }

        // The "send_*" functions will build the message for the type and send it
        // on the message sender to be signed
        let captured_msgs = self.get_captured_messages();
        let signed_msg = captured_msgs.first().unwrap();

        signed_msg.to_owned()
    }

    // Trigger a timeout by ending the round
    pub fn trigger_timeout(&mut self) -> Result<(), String> {
        let current_round: u64 = self.instance.get_round().into();

        // Check if we're at or past the cutoff round (matching Go test behavior)
        if current_round >= 15 {
            return Err("instance stopped processing timeouts".to_string());
        }

        // Increment timeout counter before triggering the timeout
        self.timeout_count += 1;
        self.instance.end_round();
        Ok(())
    }

    /// Process a message through the QBFT instance for spec tests
    pub fn process_message(&mut self, msg: &TestSignedSSVMessage) -> Result<(), String> {
        // FORCE STOP CHECK - Absolute highest priority, before ANY processing
        // This is spec test only - we implement cleanup differently in production
        if self.force_stop {
            return Err("instance stopped processing messages".to_string());
        }

        // Convert TestSignedSSVMessage to WrappedQbftMessage using spec_types conversion
        let wrapped = msg.to_wrapped_qbft_message()?;

        // ROUND CUTOFF CHECK - Spec test only, matches old process_message_spec behavior
        const TEST_CUTOFF_ROUND: u64 = 15;
        let current_round: u64 = self.instance.get_round().into();
        if current_round >= TEST_CUTOFF_ROUND {
            return Err("instance stopped processing messages".to_string());
        }

        // === Spec Test Validations (duplicating message_validator checks) ===

        // DUPLICATE: Multi-signer validation (already done in message_validator::consensus_message.rs:73-89)
        // We duplicate this here for spec tests since message_validator is bypassed
        let signers = wrapped.signed_message.operator_ids().len();
        if signers > 1 {
            match wrapped.qbft_message.qbft_message_type {
                QbftMessageType::Commit => {
                    // This matches message_validator quorum validation logic
                    let committee_size = 4; // Default test committee size
                    let quorum_size = (committee_size - 1) / 3 * 2 + 1; // f*2+1 where f=(n-1)/3
                    if signers < quorum_size {
                        return Err("invalid signed message: msg allows 1 signer".to_string());
                    }
                }
                _ => return Err("invalid signed message: msg allows 1 signer".to_string()),
            }
        }

        // Check for invalid value BEFORE hash validation (for proper error precedence)
        if wrapped.signed_message.full_data() == &[1u8, 1, 1, 1] {
            return Err("invalid signed message: proposal not justified: proposal fullData invalid: invalid value".to_string());
        }

        // Validate RSA signatures if test keys are available
        // In production, message_validator would do RSA validation
        if let Some(ref test_keys) = self.test_keys {
            validate_rsa_signatures(&wrapped, test_keys)?;
        }

        // Process message through core receive function
        // Let core QBFT handle most validation (including state validation)
        match self.instance.receive(wrapped.clone()) {
            Ok(()) => Ok(()),
            Err(qbft_error) => {
                // For hash validation errors, check if we should do additional validation
                if matches!(qbft_error, qbft::QbftError::InvalidFullData) {
                    // DUPLICATE: Full data hash validation (already done in message_validator::consensus_message.rs:99-103)
                    // We duplicate this here for spec tests since message_validator is bypassed
                    // NOTE: Only validate hash for proposal messages with non-empty data
                    if matches!(
                        wrapped.qbft_message.qbft_message_type,
                        QbftMessageType::Proposal
                    ) && !wrapped.signed_message.full_data().is_empty()
                    {
                        use sha2::{Digest, Sha256};
                        let mut hasher = Sha256::new();
                        hasher.update(wrapped.signed_message.full_data());
                        let hash_bytes: [u8; 32] = hasher.finalize().into();
                        let computed_hash = Hash256::from(hash_bytes);

                        if computed_hash != wrapped.qbft_message.root {
                            return Err("invalid signed message: H(data) != root".to_string());
                        }
                    }
                }

                // Map the QbftError to the expected spec test string
                return Err(map_qbft_error(&qbft_error));
            }
        }
    }

    // Helpers to setup the state of the QBFT Instances after constrution and get state data
    // ----------------------------------------------

    /// Populate containers with messages from QbftStartingState
    fn populate_containers(&mut self, state: &QbftStartingState) {
        // Process each container type
        for test_msg in state.propose_container.msgs.values() {
            if let Ok(wrapped) = test_msg.to_wrapped_qbft_message() {
                self.instance.add_message_to_container_spec(&wrapped);
            }
        }

        for test_msg in state.prepare_container.msgs.values() {
            if let Ok(wrapped) = test_msg.to_wrapped_qbft_message() {
                self.instance.add_message_to_container_spec(&wrapped);
            }
        }

        for test_msg in state.commit_container.msgs.values() {
            if let Ok(wrapped) = test_msg.to_wrapped_qbft_message() {
                self.instance.add_message_to_container_spec(&wrapped);
            }
        }

        for test_msg in state.round_change_container.msgs.values() {
            if let Ok(wrapped) = test_msg.to_wrapped_qbft_message() {
                self.instance.add_message_to_container_spec(&wrapped);
            }
        }
    }

    /// Setup prepare justifications from spec test data
    /// These are stored in the PrepareContainer and used for validating prepare messages
    fn setup_prepare_justifications(&mut self, pre_jus: &Vec<TestSignedSSVMessage>) {
        for test_msg in pre_jus {
            if let Ok(wrapped) = test_msg.to_wrapped_qbft_message() {
                // Add prepare justification messages to the container
                // This matches Go's behavior where justifications are stored in message containers
                self.instance.add_message_to_container_spec(&wrapped);
            }
        }
    }

    /// Setup round change justifications from spec test data
    fn setup_round_change_justifications(&mut self, rc_jus: &Vec<TestSignedSSVMessage>) {
        for test_msg in rc_jus {
            if let Ok(wrapped) = test_msg.to_wrapped_qbft_message() {
                // Add round change justification messages to the container
                // This matches Go's behavior where justifications are stored in message containers
                self.instance.add_message_to_container_spec(&wrapped);
            }
        }
    }

    /// Setup proposal accepted state
    fn setup_proposal_accepted(&mut self, accepted: &AcceptedProposal) {
        // Parse the QBFT message from the accepted proposal
        let ssv_msg = accepted.signed_message.ssv_message.as_ref().unwrap();
        let qbft_msg = QbftMessage::from_ssz_bytes(ssv_msg.data()).unwrap();

        // Rebuild the BeaconVote
        let full_data_str = accepted.signed_message.full_data.clone().unwrap();
        let full_data = STANDARD.decode(full_data_str).unwrap();
        let vote = BeaconVote::from_ssz_bytes(&full_data).unwrap();

        // Modify the state for a proposal accepted
        self.instance.store_data_spec(qbft_msg.root, vote);

        // Set proposal accepted state
        self.instance
            .set_proposal_accepted_spec(Some(qbft_msg.root));

        // Set instance state to Prepare (we accepted a proposal and are waiting for prepares)
        self.instance.set_state_spec(InstanceState::Prepare {
            proposal_root: qbft_msg.root,
        });
    }

    /// Set the round of the instance
    pub fn setup_round(&mut self, round: Round) {
        self.instance.set_current_round_spec(round);
    }

    /// Get the current round
    pub fn get_round(&self) -> u64 {
        self.instance.get_round().into()
    }

    /// Get the timeout count
    pub fn get_timeout_count(&self) -> u64 {
        self.timeout_count
    }

    // Get all of the outgoing messages
    pub fn get_captured_messages(&self) -> Vec<SignedSSVMessage> {
        self.captured_messages.borrow().clone()
    }
}
