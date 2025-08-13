use crate::utils::error_mapping::map_signed_message_error;
use openssl::hash::MessageDigest;
use openssl::pkey::{PKey, Private};
use openssl::rsa::Rsa;
use openssl::sign::Signer;
use qbft::{ConfigBuilder, InstanceHeight, InstanceState};
use qbft::{DefaultLeaderFunction, Qbft, UnsignedWrappedQbftMessage, WrappedQbftMessage};
use ssv_types::consensus::{QbftMessage, QbftMessageType, UnsignedSSVMessage};
use ssv_types::message::SignedSSVMessage;
use ssv_types::msgid::MessageId;
use ssv_types::{IndexSet, OperatorId, Round};
use ssz::{Decode, Encode};
use std::cell::RefCell;
use std::rc::Rc;

use types::{Hash256, Checkpoint};
use ssv_types::consensus::BeaconVote;

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
    last_prepared_value_bytes: Option<Vec<u8>>, // Store original StateValue bytes for FullData
    captured_messages: Rc<RefCell<Vec<SignedSSVMessage>>>, // Capture sent messages
    timeout_count: u64,                         // Track number of timeouts triggered
}

impl QbftAdapter {
    /// Create a BeaconVote from test data bytes or hash
    fn create_beacon_vote_from_bytes(data: &[u8]) -> BeaconVote {
        use sha2::{Digest, Sha256};
        
        // If data is 32 bytes, use it directly as hash, otherwise hash it
        let hash = if data.len() == 32 {
            Hash256::from_slice(data)
        } else {
            let mut hasher = Sha256::new();
            hasher.update(data);
            let hash_bytes: [u8; 32] = hasher.finalize().into();
            Hash256::from(hash_bytes)
        };
        
        BeaconVote {
            block_root: hash,
            source: Checkpoint {
                epoch: types::Epoch::new(0),
                root: hash,
            },
            target: Checkpoint {
                epoch: types::Epoch::new(1),
                root: hash,
            },
        }
    }
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

        // Calculate quorum size based on committee size (f = (n-1)/3, quorum = n - f)
        let committee_size = committee.len();
        let f = (committee_size - 1) / 3;
        let quorum_size = committee_size - f;

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
                let serialized = msg.unsigned_message.ssv_message.as_ssz_bytes();
                let pkey = PKey::from_rsa(rsa_key.clone()).expect("Valid RSA key");
                let mut signer =
                    Signer::new(MessageDigest::sha256(), &pkey).expect("Create signer");
                signer.update(&serialized).expect("Update signer");
                let mut sig = [0u8; 256];
                let len = signer.sign(&mut sig).expect("Sign message");
                assert_eq!(len, 256, "Signature must be 256 bytes");
                sig
            } else {
                [0u8; 256]
            };

            let signed = SignedSSVMessage::new(
                vec![signature],
                vec![op_id],
                msg.unsigned_message.ssv_message,
                vec![], // Full data handled separately in create_message
            )
            .expect("Failed to create signed message");

            captured_clone.borrow_mut().push(signed);
        });

        let mut instance = Qbft::new(
            config,
            Self::create_beacon_vote_from_bytes(&[]),
            identifier,
            mock_handler,
        );

        // Set the current round if provided (for spec tests)
        if let Some(round) = state.round {
            instance.set_current_round_spec(round);
        }

        Self {
            instance,
            operator_rsa_key: state.operator_rsa_key,
            operator_id,
            last_prepared_value_bytes: None,
            captured_messages: captured,
            timeout_count: 0,
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
                use openssl::sha::sha256;
                let prepared_value_hash = sha256(state_value);
                let prepared_value = Hash256::from_slice(&prepared_value_hash);

                // Create test data to store as the full data
                let bytes: &[u8] = prepared_value.as_ref();
                let dummy_vote = Self::create_beacon_vote_from_bytes(bytes);

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
            use openssl::sha::sha256;
            let hash_bytes = sha256(data);
            let hash = Hash256::from_slice(&hash_bytes);

            // Store dummy data for the proposal
            let bytes: &[u8] = hash.as_ref();
            let dummy_vote = Self::create_beacon_vote_from_bytes(bytes);
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

    fn sign_message(&self, unsigned: UnsignedSSVMessage) -> SignedSSVMessage {
        self.sign_message_with_full_data(unsigned, vec![])
    }

    fn sign_message_with_full_data(
        &self,
        unsigned: UnsignedSSVMessage,
        full_data: Vec<u8>,
    ) -> SignedSSVMessage {
        let signature = if let Some(ref rsa_key) = self.operator_rsa_key {
            // Real signing - same as Go implementation
            let serialized = unsigned.ssv_message.as_ssz_bytes();
            let pkey = PKey::from_rsa(rsa_key.clone()).expect("Valid RSA key");
            let mut signer = Signer::new(MessageDigest::sha256(), &pkey).expect("Create signer");
            signer.update(&serialized).expect("Update signer");
            let mut sig = [0u8; 256];
            let len = signer.sign(&mut sig).expect("Sign message");
            assert_eq!(len, 256, "Signature must be 256 bytes");
            sig
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

    /// Common initialization logic for adapter from test state
    fn initialize_from_state(
        operator_id: OperatorId,
        height: InstanceHeight,
        round: Round,
        identifier: MessageId,
        committee: Option<Vec<OperatorId>>,
        test_keys: &crate::utils::test_keys::TestKeySet,
    ) -> Self {
        let committee = committee
            .unwrap_or_else(|| vec![1, 2, 3, 4].into_iter().map(OperatorId::from).collect());

        let starting_state = QbftStartingState {
            height: Some(height),
            identifier: Some(identifier),
            committee: Some(committee),
            operator_id: Some(operator_id),
            operator_rsa_key: test_keys.operator_keys.get(&operator_id).cloned(),
            round: Some(round),
        };

        let mut adapter = Self::new_with_state(starting_state);

        // Clear any messages sent during creation
        adapter.captured_messages.borrow_mut().clear();

        // Initialize timeout count to 0 for tests
        adapter.timeout_count = 0;

        adapter
    }

    /// Create adapter from timeout test Pre state
    pub fn from_timeout_pre(
        pre: &crate::qbft::timeout::TimeoutTestPre,
        test_keys: &crate::utils::test_keys::TestKeySet,
    ) -> Result<Self, String> {
        use ssv_types::consensus::QbftMessage;
        use ssz::Decode;

        // Extract basic config from pre state
        let operator_id = OperatorId::from(pre.state.committee_member.operator_id);
        let height = InstanceHeight::from(pre.state.height as usize);
        let round = Round::from(pre.state.round);
        let identifier = MessageId::from(
            <[u8; 56]>::try_from(pre.state.id.as_slice())
                .map_err(|_| "Invalid identifier length")?,
        );

        // Use common initialization
        let mut adapter = Self::initialize_from_state(
            operator_id,
            height,
            round,
            identifier,
            None, // Use default committee
            test_keys,
        );

        // Now set the ProposalAcceptedForCurrentRound if present
        if let Some(ref accepted) = pre.state.proposal_accepted_for_current_round {
            // Parse the QBFT message from the accepted proposal
            let ssv_msg = accepted
                .signed_message
                .ssv_message
                .as_ref()
                .ok_or_else(|| "ProposalAcceptedForCurrentRound has null SSVMessage".to_string())?;
            let qbft_msg = QbftMessage::from_ssz_bytes(ssv_msg.data())
                .map_err(|e| format!("Failed to decode accepted proposal: {:?}", e))?;

            // Set proposal accepted state
            adapter
                .instance
                .set_proposal_accepted_spec(true, Some(qbft_msg.root));

            // Set instance state to Prepare (we accepted a proposal and are waiting for prepares)
            adapter.instance.set_state_spec(InstanceState::Prepare {
                proposal_root: qbft_msg.root,
            });
        }

        Ok(adapter)
    }

    /// Process a message through the QBFT instance for spec tests
    /// This method converts the test message format and feeds it to the core
    pub fn process_message(
        &mut self,
        msg: &crate::types::TestSignedSSVMessage,
    ) -> Result<(), String> {
        // Convert TestSignedSSVMessage to WrappedQbftMessage
        let wrapped = self.convert_test_message(msg)?;

        // Process through the core QBFT instance
        self.instance.process_message_spec(wrapped)?;

        Ok(())
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

        // Use common initialization
        let mut adapter = Self::initialize_from_state(
            operator_id,
            height,
            round,
            identifier,
            committee,
            test_keys,
        );

        // Set LastPrepared if present
        if pre.state.last_prepared_round > 0 {
            if let Some(ref value_bytes) = pre.state.last_prepared_value {
                // Store the original bytes for FullData
                adapter.last_prepared_value_bytes = Some(value_bytes.clone());

                // Hash the value
                use openssl::sha::sha256;
                let prepared_hash = sha256(value_bytes);
                let prepared_value = Hash256::from_slice(&prepared_hash);

                // Create dummy vote for storage
                let bytes: &[u8] = prepared_value.as_ref();
                let dummy_vote = Self::create_beacon_vote_from_bytes(bytes);

                adapter.instance.set_last_prepared_spec(
                    Round::from(pre.state.last_prepared_round),
                    prepared_value,
                    dummy_vote,
                );
            }
        }

        // Set ProposalAcceptedForCurrentRound if present
        if let Some(ref accepted) = pre.state.proposal_accepted_for_current_round {
            // Parse the QBFT message from the accepted proposal
            let ssv_msg = accepted
                .signed_message
                .ssv_message
                .as_ref()
                .ok_or_else(|| "ProposalAcceptedForCurrentRound has null SSVMessage".to_string())?;
            let qbft_msg = QbftMessage::from_ssz_bytes(ssv_msg.data())
                .map_err(|e| format!("Failed to decode accepted proposal: {:?}", e))?;

            // Set proposal accepted state
            adapter
                .instance
                .set_proposal_accepted_spec(true, Some(qbft_msg.root));

            // Set instance state to Prepare (we accepted a proposal and are waiting for prepares)
            adapter.instance.set_state_spec(InstanceState::Prepare {
                proposal_root: qbft_msg.root,
            });
        }

        // Populate containers from pre.state
        adapter.populate_containers(&pre.state)?;

        // Set forceStop if needed
        if pre.force_stop {
            // TODO: Handle force_stop in the adapter or test layer
            // The Go implementation has forceStop but we handle it differently
        }

        Ok(adapter)
    }

    /// Populate containers with messages from pre-state
    fn populate_containers(
        &mut self,
        state: &crate::qbft::message_processing::MessageProcessingState,
    ) -> Result<(), String> {
        // Process each container type
        for (_key, test_msg) in &state.propose_container.msgs {
            if let Ok(wrapped) = self.convert_test_message(test_msg) {
                self.instance.add_message_to_container_spec(&wrapped);
            }
        }

        for (_key, test_msg) in &state.prepare_container.msgs {
            if let Ok(wrapped) = self.convert_test_message(test_msg) {
                self.instance.add_message_to_container_spec(&wrapped);
            }
        }

        for (_key, test_msg) in &state.commit_container.msgs {
            if let Ok(wrapped) = self.convert_test_message(test_msg) {
                self.instance.add_message_to_container_spec(&wrapped);
            }
        }

        for (_key, test_msg) in &state.round_change_container.msgs {
            if let Ok(wrapped) = self.convert_test_message(test_msg) {
                self.instance.add_message_to_container_spec(&wrapped);
            }
        }

        Ok(())
    }

    /// Convert a test message to the internal QBFT format
    fn convert_test_message(
        &self,
        test_msg: &crate::types::TestSignedSSVMessage,
    ) -> Result<WrappedQbftMessage, String> {
        // Get the SSVMessage from the test message
        let ssv_message = test_msg
            .ssv_message
            .as_ref()
            .ok_or_else(|| "TestSignedSSVMessage has null SSVMessage".to_string())?;

        // Decode QBFT message first to check type
        let qbft_message = QbftMessage::from_ssz_bytes(ssv_message.data())
            .map_err(|e| format!("Failed to decode QBFT message: {:?}", e))?;

        // Convert signatures - test messages have base64 encoded signatures
        let signatures: Vec<[u8; 256]> = test_msg
            .signatures
            .iter()
            .map(|sig_str| {
                use base64::Engine;
                let sig_bytes = base64::engine::general_purpose::STANDARD
                    .decode(sig_str)
                    .map_err(|e| format!("Failed to decode signature: {:?}", e))?;
                if sig_bytes.len() != 256 {
                    return Err(format!("Invalid signature length: {}", sig_bytes.len()));
                }
                let mut sig = [0u8; 256];
                sig.copy_from_slice(&sig_bytes);
                Ok(sig)
            })
            .collect::<Result<Vec<_>, String>>()?;

        // Convert operator IDs
        let operator_ids: Vec<OperatorId> = test_msg
            .operator_ids
            .as_ref()
            .ok_or_else(|| "TestSignedSSVMessage has null OperatorIDs".to_string())?
            .clone();

        // Handle full_data conversion based on message type
        let full_data = if let Some(data_str) = &test_msg.full_data {
            use base64::Engine;
            let raw_data = base64::engine::general_purpose::STANDARD
                .decode(data_str)
                .map_err(|e| format!("Failed to decode full_data: {:?}", e))?;

            if !raw_data.is_empty() && qbft_message.qbft_message_type == QbftMessageType::Proposal {
                // For proposals, use the raw test data directly
                // Create BeaconVote from test data
                raw_data.clone()
            } else {
                // For non-proposals or empty data, keep as is
                raw_data
            }
        } else {
            vec![]
        };

        // Create SignedSSVMessage - this may fail for invalid messages (e.g., duplicate signers)
        // We need to propagate this error so it can be matched against expected errors
        let signed_ssv_message =
            match SignedSSVMessage::new(signatures, operator_ids, ssv_message.clone(), full_data) {
                Ok(msg) => msg,
                Err(e) => {
                    // Convert the error to match Go's format using centralized mapping
                    let error_str = map_signed_message_error(&e);
                    return Err(error_str);
                }
            };

        // Create WrappedQbftMessage (we already decoded qbft_message above)
        Ok(WrappedQbftMessage {
            signed_message: signed_ssv_message,
            qbft_message,
        })
    }
}
