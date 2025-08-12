use openssl::hash::MessageDigest;
use openssl::pkey::{PKey, Private};
use openssl::rsa::Rsa;
use openssl::sign::Signer;
use qbft::{ConfigBuilder, InstanceHeight, InstanceState};
use qbft::{DefaultLeaderFunction, Qbft, UnsignedWrappedQbftMessage, WrappedQbftMessage};
use ssv_types::consensus::{BeaconVote, QbftMessage, QbftMessageType, UnsignedSSVMessage};
use ssv_types::message::SignedSSVMessage;
use ssv_types::msgid::MessageId;
use ssv_types::{IndexSet, OperatorId, Round};
use ssz::{Decode, Encode};
use std::cell::RefCell;
use std::rc::Rc;

use types::{FixedBytesExtended, Hash256};

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
    timeout_count: u64, // Track number of timeouts triggered
    // Store initial state data for root calculation
    initial_height: u64,
    initial_id: Vec<u8>,
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

        // Calculate quorum size based on committee size (f = (n-1)/3, quorum = n - f)
        let committee_size = committee.len();
        let f = (committee_size - 1) / 3;
        let quorum_size = committee_size - f;

        // Build config with actual committee
        // Set max_rounds high enough for all tests (round 15 needs at least 16)
        let config = ConfigBuilder::new(operator_id, height, committee)
            .with_quorum_size(quorum_size)
            .with_max_rounds(100)  // Support very high rounds for testing
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
                let mut signer = Signer::new(MessageDigest::sha256(), &pkey).expect("Create signer");
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
            BeaconVote {
                block_root: Hash256::zero(),
                source: types::Checkpoint {
                    epoch: types::Epoch::new(0),
                    root: Hash256::zero(),
                },
                target: types::Checkpoint {
                    epoch: types::Epoch::new(0),
                    root: Hash256::zero(),
                },
            },
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
            initial_height: *height as u64,
            initial_id: {
                let id = state.identifier.unwrap_or(MessageId::from([0u8; 56]));
                let id_bytes: [u8; 56] = id.into();
                id_bytes.to_vec()
            },
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

                // Create a dummy BeaconVote to store as the full data
                let dummy_vote = BeaconVote {
                    block_root: prepared_value,
                    source: types::Checkpoint {
                        epoch: types::Epoch::new(0),
                        root: Hash256::zero(),
                    },
                    target: types::Checkpoint {
                        epoch: types::Epoch::new(0),
                        root: Hash256::zero(),
                    },
                };

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
            let dummy_vote = BeaconVote {
                block_root: hash,
                source: types::Checkpoint {
                    epoch: types::Epoch::new(0),
                    root: Hash256::zero(),
                },
                target: types::Checkpoint {
                    epoch: types::Epoch::new(0),
                    root: Hash256::zero(),
                },
            };
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
    
    /// Get a mutable reference to the QBFT instance
    pub fn get_instance(&mut self) -> &mut Qbft<DefaultLeaderFunction, BeaconVote, MockHandler> {
        &mut self.instance
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
    
    /// Build the current state for root calculation (matching Go's format)
    pub fn build_state_for_root(&self, pre_state: &crate::qbft::timeout::TimeoutTestPre) -> crate::qbft::timeout::QbftInstanceState {
        
        // Build state structure matching the test format
        // For timeout tests, the main thing that changes is the round number
        // We reuse most of the pre state but update the round
        let mut state = pre_state.state.clone();
        
        // Update the round to the current round after timeout
        state.round = self.instance.get_round().into();
        
        // Clear proposal accepted (gets cleared on round change)
        state.proposal_accepted_for_current_round = None;
        
        // For timeout tests, containers are typically empty after round change
        // But to be safe, preserve what was in pre state
        
        state
    }
    
    /// Create adapter from timeout test Pre state
    pub fn from_timeout_pre(
        pre: &crate::qbft::timeout::TimeoutTestPre,
        test_keys: &crate::utils::test_keys::TestKeySet,
    ) -> Result<Self, String> {
        use ssz::Decode;
        use ssv_types::consensus::QbftMessage;
        
        // Extract basic config from pre state
        let operator_id = OperatorId::from(pre.state.committee_member.operator_id);
        let height = InstanceHeight::from(pre.state.height as usize);
        let round = Round::from(pre.state.round);
        let identifier = MessageId::from(
            <[u8; 56]>::try_from(pre.state.id.as_slice())
                .map_err(|_| "Invalid identifier length")?
        );
        
        // Build starting state
        let starting_state = QbftStartingState {
            height: Some(height),
            identifier: Some(identifier),
            committee: Some(vec![1, 2, 3, 4].into_iter().map(OperatorId::from).collect()),
            operator_id: Some(operator_id),
            operator_rsa_key: test_keys.operator_keys.get(&operator_id).cloned(),
            round: Some(round),
        };
        
        // Create adapter with basic state
        let mut adapter = Self::new_with_state(starting_state);
        
        // Clear any messages sent during creation (like auto-proposal)
        adapter.captured_messages.borrow_mut().clear();
        
        // Initialize timeout count to 0 for tests
        adapter.timeout_count = 0;
        
        // Store the initial state data
        adapter.initial_height = pre.state.height;
        adapter.initial_id = pre.state.id.clone();
        
        // Now set the ProposalAcceptedForCurrentRound if present
        if let Some(ref accepted) = pre.state.proposal_accepted_for_current_round {
            // Parse the QBFT message from the accepted proposal
            let ssv_msg = accepted.signed_message.ssv_message.as_ref()
                .ok_or_else(|| "ProposalAcceptedForCurrentRound has null SSVMessage".to_string())?;
            let qbft_msg = QbftMessage::from_ssz_bytes(ssv_msg.data())
                .map_err(|e| format!("Failed to decode accepted proposal: {:?}", e))?;
            
            // Set proposal accepted state
            adapter.instance.set_proposal_accepted_spec(true, Some(qbft_msg.root));
            
            // Set instance state to Prepare (we accepted a proposal and are waiting for prepares)
            adapter.instance.set_state_spec(InstanceState::Prepare { proposal_root: qbft_msg.root });
        }
        
        Ok(adapter)
    }
    
    /// Setup complete state from MessageProcessingState for spec tests
    pub fn setup_from_message_processing_state(
        &mut self,
        state: &crate::qbft::message_processing::MessageProcessingState,
        _start_value: &[u8],
    ) -> Result<(), String> {
        use ssz::Decode;
        use ssv_types::consensus::QbftMessage;
        
        // Set the round
        self.instance.set_current_round_spec(Round::from(state.round));
        
        // Set last prepared state if present
        if state.last_prepared_round > 0 {
            if let Some(ref value_bytes) = state.last_prepared_value {
                // Store the original bytes for FullData
                self.last_prepared_value_bytes = Some(value_bytes.clone());
                
                // Hash the value
                use openssl::sha::sha256;
                let prepared_hash = sha256(value_bytes);
                let prepared_value = Hash256::from_slice(&prepared_hash);
                
                // Create dummy vote for storage
                let dummy_vote = BeaconVote {
                    block_root: prepared_value,
                    source: types::Checkpoint {
                        epoch: types::Epoch::new(0),
                        root: Hash256::zero(),
                    },
                    target: types::Checkpoint {
                        epoch: types::Epoch::new(0),
                        root: Hash256::zero(),
                    },
                };
                
                self.instance.set_last_prepared_spec(
                    Round::from(state.last_prepared_round),
                    prepared_value,
                    dummy_vote,
                );
            }
        }
        
        // Set proposal accepted state
        if let Some(ref accepted) = state.proposal_accepted_for_current_round {
            let ssv_msg = accepted.signed_message.ssv_message.as_ref()
                .ok_or_else(|| "ProposalAcceptedForCurrentRound has null SSVMessage".to_string())?;
            let qbft_msg = QbftMessage::from_ssz_bytes(ssv_msg.data())
                .map_err(|e| format!("Failed to decode accepted proposal: {:?}", e))?;
            
            self.instance.set_proposal_accepted_spec(true, Some(qbft_msg.root));
            
            // Note: We can't directly add TestSignedSSVMessage to the container
            // The container expects SignedSSVMessage from ssv_types
            // For now, just set the proposal accepted state which is what matters for timeout tests
        }
        
        // Populate containers from state
        // Note: Container population is commented out because we have TestSignedSSVMessage
        // but the containers expect SignedSSVMessage from ssv_types.
        // For timeout tests, we only need the proposal accepted state which is set above.
        
        // Set decided state if needed
        if state.decided {
            self.instance.set_state_spec(InstanceState::Complete);
        }
        
        Ok(())
    }
}

// Simple mock function factory
pub fn create_mock_handler() -> impl FnMut(UnsignedWrappedQbftMessage) {
    |_msg: UnsignedWrappedQbftMessage| {
        // Mock implementation - just ignore the message
    }
}
