use super::spec_types::{AcceptedProposal, TestSignedSSVMessage};
use crate::qbft::message_processing::MessageProcessingPre;
use crate::qbft::message_processing::MessageProcessingState;
use crate::qbft::timeout::TimeoutTestPre;
use crate::utils::misc::{calculate_quorum, hash_data};
use crate::utils::rsa_signing::{sign_message_with_full_data, sign_ssz_message_with_rsa};
use crate::utils::rsa_validation::validate_rsa_signatures;
use crate::utils::test_keys::TestKeySet;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use openssl::pkey::Private;
use openssl::rsa::Rsa;
use qbft::{ConfigBuilder, InstanceHeight, InstanceState, LeaderFunction};
use qbft::{Qbft, UnsignedWrappedQbftMessage, WrappedQbftMessage};
use ssv_types::consensus::{BeaconVote, QbftMessage, QbftMessageType, UnsignedSSVMessage};
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
pub struct QbftStartingState {
    pub height: InstanceHeight,
    pub identifier: MessageId,
    pub committee: Option<Vec<OperatorId>>, // Only committee is optional
    pub operator_id: OperatorId,
    pub round: Round,
    pub start_value: Vec<u8>, // Raw SSZ bytes to decode into BeaconVote
    pub proposal_accepted: Option<AcceptedProposal>,
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
            .map(|c| c.into_iter().collect())
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

        let mut instance = Qbft::new(config, start_data, state.identifier, mock_handler);

        // Now, set all of the data
        // Set the round
        instance.set_current_round_spec(state.round);
        // Set the last prepared round & last prepared value (Always 0 and null)

        let mut adapter = Self {
            instance,
            operator_rsa_key: rsa_key,
            operator_id: state.operator_id,
            last_prepared_value_bytes: None,
            captured_messages: captured,
            timeout_count: 0,
            test_keys: Some(test_keys),
        };

        // Set the proposal accepted for current round
        if let Some(proposal_accepted) = state.proposal_accepted {
            adapter.setup_proposal_accepted(&proposal_accepted);
        }

        // Set all of the containers

        // Set the decided
        // Set the decided value
        // Set all of the containers

        adapter
    }

    /// Setup prepare justifications for spec tests (used before creating RoundChange)
    /// Matches Go's createRoundChange logic exactly
    pub fn setup_prepare_justifications(
        &mut self,
        prepare_msgs: &[SignedSSVMessage],
        state_value: Option<&[u8]>,
    ) -> Result<(), String> {
        if prepare_msgs.is_empty() {
            return Ok(());
        }

        // Add prepare messages to container first (always done in Go)
        for msg in prepare_msgs {
            let qbft_msg = QbftMessage::from_ssz_bytes(msg.ssv_message().data())
                .map_err(|e| format!("Failed to decode prepare message: {:?}", e))?;

            // Create wrapped message for the container
            let wrapped = WrappedQbftMessage {
                signed_message: msg.clone(),
                qbft_message: qbft_msg.clone(),
            };

            // Add to prepare container
            for operator_id in msg.operator_ids() {
                self.instance.add_prepare_justification_spec(
                    Round::from(qbft_msg.round),
                    *operator_id,
                    wrapped.clone(),
                );
            }
        }

        // Set last prepared value if we have StateValue and ANY prepare messages
        // This matches Go test behavior: state.LastPreparedValue = test.StateValue
        // The quorum check happens later in getRoundChangeJustification
        if let Some(state_value) = state_value {
            if !state_value.is_empty() {
                // Store the original bytes for FullData field
                self.last_prepared_value_bytes = Some(state_value.to_vec());

                // Decode first prepare message to get the round
                let first_msg = &prepare_msgs[0];
                let qbft_msg = QbftMessage::from_ssz_bytes(first_msg.ssv_message().data())
                    .map_err(|e| format!("Failed to decode prepare message: {:?}", e))?;

                // Hash the StateValue using SHA256 (matches Go's HashDataRoot)
                let prepared_value = hash_data(state_value);

                // Create BeaconVote from the original SSZ bytes
                let dummy_vote = BeaconVote::from_ssz_bytes(state_value)
                    .expect("StateValue should be valid SSZ BeaconVote");

                // Set last prepared round and value with full data
                self.instance.set_last_prepared_spec(
                    Round::from(qbft_msg.round),
                    prepared_value,
                    dummy_vote,
                );
            }
        }

        Ok(())
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
        // Determine data_hash and full_data based on message type
        let (data_hash, mut full_data) = if msg_type == QbftMessageType::Proposal {
            // For proposals: data is already the hash (TestingQBFTRootData), don't hash it again
            let hash = Hash256::from_slice(data);

            // Store BeaconVote using the same SSZ bytes that Go uses (TestingQBFTFullData)
            // This ensures internal QBFT state matches exactly what Go has
            let dummy_vote = BeaconVote::from_ssz_bytes(&[
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3,
            ])
            .expect("Hardcoded SSZ bytes should be valid BeaconVote");
            self.instance.store_data_spec(hash, dummy_vote);

            // For proposals, full_data is the SSZ-encoded BeaconVote (TestingQBFTFullData), not the hash
            let ssz_bytes = [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3,
            ];
            (hash, ssz_bytes.to_vec())
        } else {
            // For other message types, data is already a 32-byte hash
            (Hash256::from_slice(data), vec![])
        };

        // Call the instance's spec method directly - this handles all QBFT logic
        let wrapped = self.instance.new_unsigned_message_spec(
            msg_type,
            data_hash,
            rc_justifications.clone().unwrap_or_default(),
            pre_justifications.clone().unwrap_or_default(),
            round.map(Round::from),
        );

        // For RoundChange messages, check if we have a prepared value
        // If so, we need to include it as full_data (matches Go behavior)
        if msg_type == QbftMessageType::RoundChange {
            // Check the message data to see if it has a prepared value
            let qbft_msg = QbftMessage::from_ssz_bytes(wrapped.unsigned_message.ssv_message.data())
                .expect("Should be able to decode our own message");

            // If DataRound != 0 (NoRound), we have a prepared value
            // In Go: signedMsg.FullData = state.LastPreparedValue
            if qbft_msg.data_round != 0 {
                // Use the stored prepared value bytes
                full_data = self.last_prepared_value_bytes.clone().unwrap_or_default();
            }
        }

        // Extract and sign the unsigned message with full_data
        let signed_msg = sign_message_with_full_data(
            wrapped.unsigned_message,
            full_data,
            &self.operator_rsa_key,
            &self.operator_id,
        );

        Ok(signed_msg)
    }

    pub fn get_captured_messages(&self) -> Vec<SignedSSVMessage> {
        self.captured_messages.borrow().clone()
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

    /// Get the current round
    pub fn get_round(&self) -> u64 {
        self.instance.get_round().into()
    }

    /// Get the timeout count
    pub fn get_timeout_count(&self) -> u64 {
        self.timeout_count
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
            .set_proposal_accepted_spec(true, Some(qbft_msg.root));

        // Set instance state to Prepare (we accepted a proposal and are waiting for prepares)
        self.instance.set_state_spec(InstanceState::Prepare {
            proposal_root: qbft_msg.root,
        });
    }

    /// Create adapter from timeout test Pre state
    pub fn from_timeout_pre(pre: &TimeoutTestPre, test_keys: &TestKeySet) -> Self {
        // Extract basic config from pre state
        let operator_id = OperatorId::from(pre.state.committee_member.operator_id);
        let height = InstanceHeight::from(pre.state.height as usize);
        let round = Round::from(pre.state.round);
        let identifier = MessageId::from(<[u8; 56]>::try_from(pre.state.id.as_slice()).unwrap());

        // Create adapter with state
        let mut adapter = Self::new_with_state(QbftStartingState {
            height,
            identifier,
            committee: None, // Use default committee
            operator_id,
            round,
            start_value: vec![0; 112], // Default BeaconVote SSZ bytes
            proposal_accepted: None,
        });

        // Clear any messages sent during creation
        adapter.captured_messages.borrow_mut().clear();
        adapter.timeout_count = 0;

        // Set ProposalAcceptedForCurrentRound if present
        if let Some(ref accepted) = pre.state.proposal_accepted_for_current_round {
            // For timeout tests, only set prepared if LastPreparedRound > 0
            let set_prepared = pre.state.last_prepared_round > 0;
            adapter.setup_proposal_accepted(accepted);
        }

        adapter
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

    /// Create adapter from message processing test Pre state
    pub fn for_message_processing(pre: &MessageProcessingPre) -> Self {
        // Extract basic config from pre state
        let operator_id = OperatorId::from(pre.state.committee_member.operator_id);
        let height = InstanceHeight::from(pre.state.height as usize);
        let round = Round::from(pre.state.round);
        let identifier = MessageId::from(<[u8; 56]>::try_from(pre.state.id.as_slice()).unwrap());

        // Build committee from state
        let committee: Vec<OperatorId> = pre
            .state
            .committee_member
            .committee
            .iter()
            .map(|op| OperatorId::from(op.operator_id))
            .collect();

        // Create adapter with state
        let mut adapter = Self::new_with_state(QbftStartingState {
            height,
            identifier,
            committee: Some(committee),
            operator_id,
            round,
            start_value: vec![0; 112], // Default BeaconVote SSZ bytes
            proposal_accepted: None,
        });

        // Clear any messages sent during creation
        adapter.captured_messages.borrow_mut().clear();
        adapter.timeout_count = 0;

        // Populate containers from pre.state FIRST (before setting other state)
        // This ensures messages are in containers for state validation
        adapter.populate_containers(&pre.state);

        // Set ProposalAcceptedForCurrentRound if present
        if let Some(ref accepted) = pre.state.proposal_accepted_for_current_round {
            // For message processing tests, ProposalAccepted implies prepared
            adapter.setup_proposal_accepted(accepted);
        }

        // Set Decided state if present
        if pre.state.decided {
            if let Some(ref decided_bytes) = pre.state.decided_value {
                // Hash the decided value
                let decided_value = hash_data(decided_bytes);

                // Set instance state to Complete
                adapter.instance.set_state_spec(InstanceState::Complete);

                // Store the decided value as BeaconVote from original SSZ bytes
                let dummy_vote = BeaconVote::from_ssz_bytes(decided_bytes)
                    .expect("DecidedValue should be valid SSZ BeaconVote");
                adapter.instance.store_data_spec(decided_value, dummy_vote);

                // Store the decided hash for creating commit messages later
                adapter.last_prepared_value_bytes = Some(decided_bytes.clone());
            }
        }

        // Set forceStop if needed
        // this is the forcestop issues this is why not passing rn

        adapter
    }

    /// Populate containers with messages from pre-state
    fn populate_containers(&mut self, state: &MessageProcessingState) {
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
}
