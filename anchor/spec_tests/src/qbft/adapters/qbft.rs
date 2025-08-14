use super::spec_types::{AcceptedProposal, TestSignedSSVMessage};
use crate::qbft::message_processing::MessageProcessingState;
use crate::utils::misc::{calculate_quorum, create_beacon_vote_from_bytes, hash_data};
use crate::utils::rsa_signing::sign_ssz_message_with_rsa;
use crate::utils::rsa_validation::validate_rsa_signatures;
use crate::utils::test_keys::TestKeySet;
use openssl::pkey::Private;
use openssl::rsa::Rsa;
use qbft::{ConfigBuilder, InstanceHeight, InstanceState};
use qbft::{DefaultLeaderFunction, Qbft, UnsignedWrappedQbftMessage, WrappedQbftMessage};
use ssv_types::consensus::BeaconVote;
use ssv_types::consensus::{QbftMessage, QbftMessageType, UnsignedSSVMessage};
use ssv_types::message::SignedSSVMessage;
use ssv_types::msgid::MessageId;
use ssv_types::{IndexSet, OperatorId, Round};
use ssz::Decode;
use std::cell::RefCell;
use std::rc::Rc;
use types::Hash256;

/// State that we want to initialize the qbft instance with
pub struct QbftStartingState {
    pub height: Option<InstanceHeight>,
    pub identifier: Option<MessageId>,
    pub committee: Option<Vec<OperatorId>>,
    pub operator_id: Option<OperatorId>,
    pub operator_rsa_key: Option<Rsa<Private>>,
    pub round: Option<Round>,
}

// Simple mock handler type
type MockHandler = Box<dyn FnMut(UnsignedWrappedQbftMessage)>;

// Adapter over our core qbft instance
pub struct QbftAdapter {
    instance: Qbft<DefaultLeaderFunction, BeaconVote, MockHandler>,
    operator_rsa_key: Option<Rsa<Private>>,
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
        let height = state.height.unwrap_or(InstanceHeight::from(0));

        // Use identifier from state or default test identifier
        let identifier = state
            .identifier
            .clone()
            .unwrap_or_else(|| MessageId::from([0u8; 56]));

        // Use committee from state or default 4-node committee
        let committee: IndexSet<OperatorId> = state
            .committee
            .map(|c| c.into_iter().collect())
            .unwrap_or_else(|| vec![1, 2, 3, 4].into_iter().map(OperatorId::from).collect());

        // Use operator_id from state or default to operator 1
        let operator_id = state.operator_id.unwrap_or(OperatorId::from(1));

        // Calculate quorum size based on committee size
        let quorum_size = calculate_quorum(committee.len());

        // Build config with actual committee
        // Set max_rounds high enough for all tests (round 15 needs at least 16)
        let config = ConfigBuilder::new(operator_id, height, committee)
            .with_quorum_size(quorum_size)
            .with_max_rounds(100) // Support very high rounds for testing
            .build()
            .expect("Failed to build config");

        // Create a handler that captures messages
        let captured = Rc::new(RefCell::new(Vec::new()));
        let captured_clone = captured.clone();
        let rsa_key_clone = state.operator_rsa_key.clone();
        let op_id = operator_id;

        let mock_handler: MockHandler = Box::new(move |msg: UnsignedWrappedQbftMessage| {
            // Sign the message
            let signature = if let Some(ref rsa_key) = rsa_key_clone {
                sign_ssz_message_with_rsa(&msg.unsigned_message.ssv_message, rsa_key)
                    .expect("Failed to sign message")
            } else {
                [0u8; 256]
            };

            // Include full_data from the unsigned message
            let full_data = msg.unsigned_message.full_data.to_vec();

            let signed = SignedSSVMessage::new(
                vec![signature],
                vec![op_id],
                msg.unsigned_message.ssv_message,
                full_data,
            )
            .expect("Failed to create signed message");

            captured_clone.borrow_mut().push(signed);
        });

        let mut instance = Qbft::new(
            config,
            create_beacon_vote_from_bytes(&[]),
            identifier,
            mock_handler,
        );

        // Set the current round if provided (for spec tests)
        if let Some(round) = state.round {
            instance.set_current_round_spec(round);
        }

        Self {
            instance,
            operator_rsa_key: state.operator_rsa_key.clone(),
            operator_id,
            last_prepared_value_bytes: None,
            captured_messages: captured,
            timeout_count: 0,
            test_keys: None, // Will be set when creating from test state
        }
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

                // Create test data to store as the full data
                let bytes: &[u8] = prepared_value.as_ref();
                let dummy_vote = create_beacon_vote_from_bytes(bytes);

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
            // For proposals: data is raw bytes to hash (matches Go's behavior)
            let hash = hash_data(data);

            // Store dummy data for the proposal
            let bytes: &[u8] = hash.as_ref();
            let dummy_vote = create_beacon_vote_from_bytes(bytes);
            self.instance.store_data_spec(hash, dummy_vote);

            // For proposals, full_data is the original data
            (hash, data.to_vec())
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
        Ok(self.sign_message_with_full_data(wrapped.unsigned_message, full_data))
    }

    fn sign_message_with_full_data(
        &self,
        unsigned: UnsignedSSVMessage,
        full_data: Vec<u8>,
    ) -> SignedSSVMessage {
        let signature = if let Some(ref rsa_key) = self.operator_rsa_key {
            // Real signing - same as Go implementation
            sign_ssz_message_with_rsa(&unsigned.ssv_message, rsa_key)
                .expect("Failed to sign message")
        } else {
            // Fallback to mock signature if no key provided
            [0u8; 256]
        };

        SignedSSVMessage::new(
            vec![signature],
            vec![self.operator_id],
            unsigned.ssv_message,
            full_data,
        )
        .expect("Failed to create signed message")
    }

    /// Get the messages captured by the handler
    pub fn get_captured_messages(&self) -> Vec<SignedSSVMessage> {
        self.captured_messages.borrow().clone()
    }

    /// Trigger a timeout (calls end_round on the instance)
    /// Returns an error if the instance is at or past the cutoff round (15 for tests)
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

    /// Setup proposal accepted state (common logic for both test types)
    fn setup_proposal_accepted(
        &mut self,
        accepted: &AcceptedProposal,
        current_round: u64,
        set_prepared: bool,  // Whether to also set last_prepared
    ) -> Result<(), String> {
        // Parse the QBFT message from the accepted proposal
        let ssv_msg = accepted
            .signed_message
            .ssv_message
            .as_ref()
            .ok_or_else(|| "ProposalAcceptedForCurrentRound has null SSVMessage".to_string())?;
        let qbft_msg = QbftMessage::from_ssz_bytes(ssv_msg.data())
            .map_err(|e| format!("Failed to decode accepted proposal: {:?}", e))?;

        // Store the full data if present for the accepted proposal
        if let Some(ref full_data_str) = accepted.signed_message.full_data {
            use base64::Engine;
            let full_data = base64::engine::general_purpose::STANDARD
                .decode(full_data_str)
                .map_err(|e| format!("Failed to decode full_data: {:?}", e))?;
            if !full_data.is_empty() {
                // Store the data for the proposal
                let bytes: &[u8] = qbft_msg.root.as_ref();
                let dummy_vote = create_beacon_vote_from_bytes(bytes);
                self.instance.store_data_spec(qbft_msg.root, dummy_vote);

                // Store the full data bytes for later use
                self.last_prepared_value_bytes = Some(full_data);
            }
        }

        // Set proposal accepted state
        self.instance
            .set_proposal_accepted_spec(true, Some(qbft_msg.root));

        // Only set last_prepared if explicitly requested
        // For message processing tests: ProposalAccepted implies prepared
        // For timeout tests: ProposalAccepted doesn't imply prepared unless LastPreparedRound > 0
        if set_prepared {
            let bytes: &[u8] = qbft_msg.root.as_ref();
            let dummy_vote = create_beacon_vote_from_bytes(bytes);
            self.instance
                .set_last_prepared_spec(Round::from(current_round), qbft_msg.root, dummy_vote);
        }

        // Set instance state to Prepare (we accepted a proposal and are waiting for prepares)
        self.instance.set_state_spec(InstanceState::Prepare {
            proposal_root: qbft_msg.root,
        });

        Ok(())
    }

    /// Create adapter from timeout test Pre state
    pub fn from_timeout_pre(
        pre: &crate::qbft::timeout::TimeoutTestPre,
        test_keys: &crate::utils::test_keys::TestKeySet,
    ) -> Result<Self, String> {
        // Extract basic config from pre state
        let operator_id = OperatorId::from(pre.state.committee_member.operator_id);
        let height = InstanceHeight::from(pre.state.height as usize);
        let round = Round::from(pre.state.round);
        let identifier = MessageId::from(
            <[u8; 56]>::try_from(pre.state.id.as_slice())
                .map_err(|_| "Invalid identifier length")?,
        );

        // Create adapter with state
        let mut adapter = Self::new_with_state(QbftStartingState {
            height: Some(height),
            identifier: Some(identifier),
            committee: None, // Use default committee
            operator_id: Some(operator_id),
            operator_rsa_key: test_keys.operator_keys.get(&operator_id).cloned(),
            round: Some(round),
        });

        // Clear any messages sent during creation
        adapter.captured_messages.borrow_mut().clear();
        adapter.timeout_count = 0;
        adapter.test_keys = Some(test_keys.clone());

        // Set ProposalAcceptedForCurrentRound if present
        if let Some(ref accepted) = pre.state.proposal_accepted_for_current_round {
            // For timeout tests, only set prepared if LastPreparedRound > 0
            let set_prepared = pre.state.last_prepared_round > 0;
            adapter.setup_proposal_accepted(accepted, pre.state.round, set_prepared)?;
        }

        Ok(adapter)
    }

    /// Process a message through the QBFT instance for spec tests
    pub fn process_message(&mut self, msg: &TestSignedSSVMessage) -> Result<(), String> {
        // Check if instance is already decided
        if self.instance.is_decided_spec() {
            // For post-decided tests, when we receive certain messages (Proposal, Prepare),
            // we should respond with a decided (commit) message
            // This matches Go behavior where decided nodes inform others
            if let Some(ref ssv_msg) = msg.ssv_message {
                if let Ok(qbft_msg) = QbftMessage::from_ssz_bytes(ssv_msg.data()) {
                    match qbft_msg.qbft_message_type {
                        QbftMessageType::Proposal | QbftMessageType::Prepare => {
                            // Create and send a decided message
                            // Get the decided value from the instance state
                            if let Some(decided_msg) = self.instance.get_aggregated_commit() {
                                // If we have a real aggregated commit, use it
                                self.captured_messages.borrow_mut().push(decided_msg);
                            } else {
                                // Otherwise create a simple commit message for the test
                                // This handles the case where we started in decided state
                                // The test expects us to send a commit message
                                let commit_data = self.create_message(
                                    QbftMessageType::Commit,
                                    &[],   // Commit uses empty data
                                    &None, // No justifications
                                    &None,
                                    None, // Current round
                                );
                                if let Ok(commit_msg) = commit_data {
                                    self.captured_messages.borrow_mut().push(commit_msg);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            // No error - message is ignored after potentially sending decided
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
    pub fn from_message_processing_pre(
        pre: &crate::qbft::message_processing::MessageProcessingPre,
        test_keys: &crate::utils::test_keys::TestKeySet,
    ) -> Result<Self, String> {
        // Extract basic config from pre state
        let operator_id = OperatorId::from(pre.state.committee_member.operator_id);
        let height = InstanceHeight::from(pre.state.height as usize);
        let round = Round::from(pre.state.round);
        let identifier = MessageId::from(
            <[u8; 56]>::try_from(pre.state.id.as_slice())
                .map_err(|_| "Invalid identifier length")?,
        );

        // Build committee from state
        let committee: Option<Vec<OperatorId>> =
            pre.state.committee_member.committee.as_ref().map(|ops| {
                ops.iter()
                    .map(|op| OperatorId::from(op.operator_id))
                    .collect()
            });

        // Create adapter with state
        let mut adapter = Self::new_with_state(QbftStartingState {
            height: Some(height),
            identifier: Some(identifier),
            committee,
            operator_id: Some(operator_id),
            operator_rsa_key: test_keys.operator_keys.get(&operator_id).cloned(),
            round: Some(round),
        });

        // Clear any messages sent during creation
        adapter.captured_messages.borrow_mut().clear();
        adapter.timeout_count = 0;
        adapter.test_keys = Some(test_keys.clone());

        // Populate containers from pre.state FIRST (before setting other state)
        // This ensures messages are in containers for state validation
        adapter.populate_containers(&pre.state)?;

        // Set LastPrepared if present
        if pre.state.last_prepared_round > 0 {
            if let Some(ref value_bytes) = pre.state.last_prepared_value {
                // Store the original bytes for FullData
                adapter.last_prepared_value_bytes = Some(value_bytes.clone());

                // Hash the value
                let prepared_value = hash_data(value_bytes);

                // Create dummy vote for storage
                let bytes: &[u8] = prepared_value.as_ref();
                let dummy_vote = create_beacon_vote_from_bytes(bytes);

                adapter.instance.set_last_prepared_spec(
                    Round::from(pre.state.last_prepared_round),
                    prepared_value,
                    dummy_vote,
                );
            }
        }

        // Set ProposalAcceptedForCurrentRound if present
        if let Some(ref accepted) = pre.state.proposal_accepted_for_current_round {
            // For message processing tests, ProposalAccepted implies prepared
            adapter.setup_proposal_accepted(accepted, pre.state.round, true)?;
        }

        // Set Decided state if present
        if pre.state.decided {
            if let Some(ref decided_bytes) = pre.state.decided_value {
                // Hash the decided value
                let decided_value = hash_data(decided_bytes);

                // Set instance state to Complete
                adapter.instance.set_state_spec(InstanceState::Complete);

                // Store the decided value
                let bytes: &[u8] = decided_value.as_ref();
                let dummy_vote = create_beacon_vote_from_bytes(bytes);
                adapter.instance.store_data_spec(decided_value, dummy_vote);

                // Store the decided hash for creating commit messages later
                adapter.last_prepared_value_bytes = Some(decided_bytes.clone());
            }
        }

        // Set forceStop if needed
        if pre.force_stop {
            adapter.instance.force_stop_spec();
        }

        Ok(adapter)
    }

    /// Populate containers with messages from pre-state
    fn populate_containers(&mut self, state: &MessageProcessingState) -> Result<(), String> {
        // Process each container type
        for (_, test_msg) in &state.propose_container.msgs {
            if let Ok(wrapped) = test_msg.to_wrapped_qbft_message() {
                self.instance.add_message_to_container_spec(&wrapped);
            }
        }

        for (_, test_msg) in &state.prepare_container.msgs {
            if let Ok(wrapped) = test_msg.to_wrapped_qbft_message() {
                self.instance.add_message_to_container_spec(&wrapped);
            }
        }

        for (_, test_msg) in &state.commit_container.msgs {
            if let Ok(wrapped) = test_msg.to_wrapped_qbft_message() {
                self.instance.add_message_to_container_spec(&wrapped);
            }
        }

        for (_, test_msg) in &state.round_change_container.msgs {
            if let Ok(wrapped) = test_msg.to_wrapped_qbft_message() {
                self.instance.add_message_to_container_spec(&wrapped);
            }
        }

        Ok(())
    }
}
