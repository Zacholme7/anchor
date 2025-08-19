use super::spec_types::{AcceptedProposal, MessageContainer, TestSignedSSVMessage};
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
    pub committee: Option<IndexSet<OperatorId>>, // Only committee is optional
    pub operator_id: OperatorId,
    pub round: Round,
    pub start_value: Vec<u8>, // Raw SSZ bytes to decode into BeaconVote
    pub proposal_accepted: Option<AcceptedProposal>,
    pub propose_container: MessageContainer,
    pub prepare_container: MessageContainer,
    pub commit_container: MessageContainer,
    pub round_change_container: MessageContainer,
    pub round_change_justifications: Option<Vec<TestSignedSSVMessage>>,
    pub prepare_justifications: Option<Vec<TestSignedSSVMessage>>,
}

// Simple mock handler type
type MockHandler = Box<dyn FnMut(UnsignedWrappedQbftMessage)>;

// Adapter over our core qbft instance
pub struct QbftAdapter {
    instance: Qbft<TestLeaderFunction, BeaconVote, MockHandler>,
    operator_rsa_key: Rsa<Private>,
    operator_id: OperatorId,
    // Store original state value bytes for fulldata
    last_prepared_value_bytes: Option<Vec<u8>>,
    // Capture send messages
    captured_messages: Rc<RefCell<Vec<SignedSSVMessage>>>, // Capture sent messages
    // Track number of timeouts triggered
    timeout_count: u64, // Track number of timeouts triggered
    // Store test keys for validation
    test_keys: Option<TestKeySet>, // Store test keys for validation
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

        let mut instance = Qbft::new(config, start_data, state.identifier.clone(), mock_handler);

        // Set the round
        instance.set_current_round_spec(state.round);

        let mut adapter = Self {
            instance,
            operator_rsa_key: rsa_key,
            operator_id: state.operator_id,
            last_prepared_value_bytes: None,
            captured_messages: captured,
            timeout_count: 0,
            test_keys: Some(test_keys),
        };

        // Clear any messages sent during creation
        adapter.captured_messages.borrow_mut().clear();
        adapter.timeout_count = 0;

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

        // Populate all containers
        adapter.populate_containers(&state);

        adapter
    }

    /// Create a new SignedSSVMessage using the instance
    pub fn create_message(
        &mut self,
        msg_type: QbftMessageType,
        data: &[u8],
        rc_justifications: &Option<Vec<SignedSSVMessage>>,
        pre_justifications: &Option<Vec<SignedSSVMessage>>,
        round: Option<u64>,
    ) -> Result<SignedSSVMessage, bool> {
        // have to store the data
        //self.instance.store_data_spec(hash, dummy_vote);

        /*
                match msg_type {
                    QbftMessageType::Proposal => self.instance.send_proposal(),
                    QbftMessageType::Prepare => self.instance.send_prepare(),
                    QbftMessageType::Commit => self.instance.send_commit()
                    QbftMessageType::RoundChange => self.instance.send_round_change(),
                }
        */

        // Receive the message here
        let unsigned_msg = self.get_captured_messages();
        let unsigned_message = unsigned_msg.first().unwrap();
        // Sign it, could even put this in the receive
        // let signed = sign(unsigned_msg);

        //Ok(signed_msg)
        todo!()
    }

    pub fn trigger_timeout(&mut self) -> Result<(), String> {
        const TEST_CUTOFF_ROUND: u64 = 15;

        let current_round: u64 = self.instance.get_round().into();

        // Check if we're at or past the cutoff round (matching Go test behavior)
        if current_round >= TEST_CUTOFF_ROUND {
            return Err("instance stopped processing timeouts".to_string());
        }

        // Increment timeout counter before triggering the timeout
        self.timeout_count += 1;

        self.instance.end_round();
        Ok(())
    }

    /// Process a message through the QBFT instance for spec tests
    pub fn process_message(&mut self, msg: &TestSignedSSVMessage) -> Result<(), String> {
        // Check if instance is already decided
        if self.instance.is_decided_spec() {
            // For post-decided tests, proposals should return an error
            if let Some(ref ssv_msg) = msg.ssv_message {
                if let Ok(qbft_msg) = QbftMessage::from_ssz_bytes(ssv_msg.data()) {
                    if matches!(qbft_msg.qbft_message_type, QbftMessageType::Proposal) {
                        // Proposals after decided should return an error
                        return Err(
                            "invalid signed message: proposal is not valid with current state"
                                .to_string(),
                        );
                    }
                }
            }
            // Non-proposal messages are silently ignored after decided
            return Ok(());
        }

        // Convert TestSignedSSVMessage to WrappedQbftMessage using spec_types conversion
        let wrapped = msg.to_wrapped_qbft_message()?;

        // Validate RSA signatures if test keys are available
        // In production, message_validator would do RSA validation
        if let Some(ref test_keys) = self.test_keys {
            validate_rsa_signatures(&wrapped, test_keys)?;
        }

        // Process through the core QBFT instance - it handles all protocol validation
        match self.instance.process_message_spec(wrapped) {
            Ok(()) => Ok(()),
            Err(qbft_error) => {
                // Map the QbftError to the expected spec test string
                use crate::utils::error_mapping::map_qbft_error;
                Err(map_qbft_error(&qbft_error))
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
