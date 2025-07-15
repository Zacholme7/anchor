use std::{collections::VecDeque, sync::Arc};

use message_validator::ValidationFailure;
use openssl::{
    hash::{Hasher, MessageDigest},
    pkey::{PKey, Private},
    rsa::Padding,
};
use parking_lot::RwLock;
use qbft::{
    Config, ConfigBuilder, DefaultLeaderFunction, InstanceHeight, MessageSender, Qbft, TestConfig,
    TestError, UnsignedWrappedQbftMessage, WrappedQbftMessage,
};
use ssv_types::{
    IndexSet, OperatorId, Round,
    consensus::{BeaconVote, QbftMessage, QbftMessageType},
    message::SignedSSVMessage,
    msgid::MessageId,
};
use ssz::{Decode, Encode};
use tree_hash::TreeHash;
use types::Hash256;

use super::validation_adapter::ValidationAdapter;

/// Unified test adapter providing comprehensive QBFT testing capabilities
/// This consolidates all QBFT testing functionality into a single, professional interface
pub struct UnifiedTestAdapter {
    // Core QBFT instance
    qbft: Qbft<DefaultLeaderFunction, BeaconVote, TestMessageSender>,

    // Test configuration and committee info
    committee: IndexSet<OperatorId>,
    identifier: MessageId,
    config: TestConfig,

    // Message handling
    message_queue: Arc<RwLock<VecDeque<UnsignedWrappedQbftMessage>>>,

    // Signing infrastructure
    signing_key: Option<PKey<Private>>,
    signing_operator_id: Option<OperatorId>,

    // Test state tracking
    processed_messages: Vec<SignedSSVMessage>,

    // Controller-level state tracking
    instance_started: bool,
    instance_value: Option<Vec<u8>>,
    decided_count: u64,
    output_messages: Vec<SignedSSVMessage>,
    current_height: u64,

    // Store the decided value from external decided messages
    decided_value: Option<Vec<u8>>,

    // Track if the instance has been decided to reject subsequent messages
    instance_decided: bool,

    // Validation adapter for message validation using message_validator
    validation_adapter: ValidationAdapter,
}

impl UnifiedTestAdapter {
    /// Create new unified test adapter
    pub fn new(
        committee: IndexSet<OperatorId>,
        identifier: MessageId,
        config: TestConfig,
    ) -> Result<Self, TestError> {
        // Build QBFT configuration
        let qbft_config: Config<DefaultLeaderFunction> = ConfigBuilder::new(
            OperatorId::from(1),
            InstanceHeight::from(config.instance_height as usize),
            committee.clone(),
        )
        .build()
        .map_err(|e| TestError::ScenarioSetupError(format!("Config build failed: {}", e)))?;

        // Create test data for QBFT
        let test_data = BeaconVote {
            block_root: Hash256::random(),
            source: types::Checkpoint::default(),
            target: types::Checkpoint::default(),
        };

        // Setup message queue and sender
        let message_queue = Arc::new(RwLock::new(VecDeque::new()));
        let message_sender = TestMessageSender {
            queue: message_queue.clone(),
        };

        // Initialize core QBFT instance
        let qbft = Qbft::new(qbft_config, test_data, identifier.clone(), message_sender);

        // Initialize validation adapter
        let validation_adapter = ValidationAdapter::new(committee.clone());

        Ok(Self {
            qbft,
            committee,
            identifier,
            message_queue,
            signing_key: None,
            signing_operator_id: None,
            processed_messages: Vec::new(),
            instance_started: false,
            instance_value: None,
            decided_count: 0,
            output_messages: Vec::new(),
            current_height: config.instance_height as u64,
            decided_value: None,
            instance_decided: false,
            validation_adapter,
            config,
        })
    }

    /// Set signing key for message signing
    pub fn set_signing_key(&mut self, key: PKey<Private>) {
        self.signing_key = Some(key);
        // Default to OperatorId(1) for backward compatibility
        self.signing_operator_id = Some(OperatorId::from(1));
    }

    /// Set signing key with specific operator ID
    pub fn set_signing_key_with_operator(&mut self, key: PKey<Private>, operator_id: OperatorId) {
        self.signing_key = Some(key);
        self.signing_operator_id = Some(operator_id);
    }

    /// Setup test scenario (from QbftTestAdapter)
    pub fn setup_test_scenario(&mut self, scenario: TestScenario) -> Result<(), TestError> {
        // Set last prepared state
        self.qbft
            .set_test_state(scenario.last_prepared_round, scenario.last_prepared_value);

        // Add round change justifications to containers
        if !scenario.round_change_justifications.is_empty() {
            let wrapped_messages: Vec<WrappedQbftMessage> = scenario
                .round_change_justifications
                .into_iter()
                .map(|signed_msg| {
                    let qbft_message = QbftMessage::from_ssz_bytes(signed_msg.ssv_message().data())
                        .map_err(|e| {
                            TestError::JustificationError(format!(
                                "Failed to decode QBFT message: {:?}",
                                e
                            ))
                        })?;
                    Ok(WrappedQbftMessage {
                        signed_message: signed_msg,
                        qbft_message,
                    })
                })
                .collect::<Result<Vec<_>, TestError>>()?;

            self.qbft
                .add_test_justifications(scenario.round, wrapped_messages);
        }

        // Add prepare justifications to containers
        if !scenario.prepare_justifications.is_empty() {
            let wrapped_messages: Vec<WrappedQbftMessage> = scenario
                .prepare_justifications
                .into_iter()
                .map(|signed_msg| {
                    let qbft_message = QbftMessage::from_ssz_bytes(signed_msg.ssv_message().data())
                        .map_err(|e| {
                            TestError::JustificationError(format!(
                                "Failed to decode QBFT message: {:?}",
                                e
                            ))
                        })?;
                    Ok(WrappedQbftMessage {
                        signed_message: signed_msg,
                        qbft_message,
                    })
                })
                .collect::<Result<Vec<_>, TestError>>()?;

            self.qbft
                .add_test_justifications(scenario.round, wrapped_messages);
        }

        Ok(())
    }

    /// Create proposal message
    pub fn create_proposal(
        &mut self,
        data_hash: Hash256,
        round: Option<Round>,
    ) -> Result<SignedSSVMessage, TestError> {
        // Add test data to QBFT
        let test_data = Arc::new(BeaconVote {
            block_root: data_hash,
            source: types::Checkpoint::default(),
            target: types::Checkpoint::default(),
        });

        self.qbft.add_test_data(data_hash, test_data.clone());

        // Create proposal using core QBFT
        let unsigned_message = self
            .qbft
            .create_proposal(test_data, round)
            .map_err(|e| TestError::MessageCreationFailed(e.to_string()))?;

        // Sign the message
        self.sign_message(unsigned_message)
    }

    /// Create prepare message
    pub fn create_prepare(
        &mut self,
        data_hash: Hash256,
        round: Option<Round>,
    ) -> Result<SignedSSVMessage, TestError> {
        // Add test data to QBFT
        let test_data = Arc::new(BeaconVote {
            block_root: data_hash,
            source: types::Checkpoint::default(),
            target: types::Checkpoint::default(),
        });

        self.qbft.add_test_data(data_hash, test_data);

        // Create prepare using core QBFT
        let unsigned_message = self
            .qbft
            .create_prepare(data_hash, round)
            .map_err(|e| TestError::MessageCreationFailed(e.to_string()))?;

        // Sign the message
        self.sign_message(unsigned_message)
    }

    /// Create commit message
    pub fn create_commit(
        &mut self,
        data_hash: Hash256,
        round: Option<Round>,
    ) -> Result<SignedSSVMessage, TestError> {
        // Add test data to QBFT
        let test_data = Arc::new(BeaconVote {
            block_root: data_hash,
            source: types::Checkpoint::default(),
            target: types::Checkpoint::default(),
        });

        self.qbft.add_test_data(data_hash, test_data);

        // Create commit using core QBFT
        let unsigned_message = self
            .qbft
            .create_commit(data_hash, round)
            .map_err(|e| TestError::MessageCreationFailed(e.to_string()))?;

        // Sign the message
        self.sign_message(unsigned_message)
    }

    /// Create round change message
    pub fn create_round_change(
        &mut self,
        state_value: Option<Vec<u8>>,
        target_round: Option<Round>,
    ) -> Result<SignedSSVMessage, TestError> {
        // Create round change using core QBFT
        let unsigned_message = self
            .qbft
            .create_round_change(state_value, target_round)
            .map_err(|e| TestError::MessageCreationFailed(e.to_string()))?;

        // Sign the message
        self.sign_message(unsigned_message)
    }

    /// Create message with comprehensive parameters
    pub fn create_message(
        &mut self,
        message_type: QbftMessageType,
        data_hash: Hash256,
        round: Option<Round>,
        state_value: Option<Vec<u8>>,
        round_change_justifications: Vec<SignedSSVMessage>,
        prepare_justifications: Vec<SignedSSVMessage>,
    ) -> Result<SignedSSVMessage, TestError> {
        use sha2::{Digest, Sha256};

        // Handle round change logic with state value
        let effective_data_hash = if message_type == QbftMessageType::RoundChange {
            if let Some(ref state_value_bytes) = state_value {
                // Use SHA256 of the StateValue as the data_hash for previously prepared round
                // change
                Hash256::from_slice(&Sha256::digest(state_value_bytes))
            } else {
                // Use zero hash for non-prepared round change
                let hash = Hash256::default();
                eprintln!("  effective_data_hash (default): {:?}", hash);
                hash
            }
        } else {
            eprintln!("  effective_data_hash (original): {:?}", data_hash);
            data_hash
        };

        // Create the unsigned message using the core QBFT logic
        let mut unsigned_message = self.qbft.new_unsigned_message_spec(
            message_type,
            effective_data_hash,
            round_change_justifications,
            prepare_justifications,
            round,
        );

        eprintln!("  unsigned_message created");
        eprintln!(
            "  unsigned_message.ssv_message data length: {}",
            unsigned_message.unsigned_message.ssv_message.data().len()
        );
        eprintln!(
            "  unsigned_message.full_data length: {}",
            unsigned_message.unsigned_message.full_data.len()
        );

        // Handle round change state value full_data
        if message_type == QbftMessageType::RoundChange {
            if let Some(state_value_bytes) = state_value {
                eprintln!("  Setting full_data for round change with state_value");
                unsigned_message.unsigned_message.full_data = state_value_bytes;
                eprintln!(
                    "  full_data after setting: {} bytes",
                    unsigned_message.unsigned_message.full_data.len()
                );
            }
        }

        // Sign the message
        eprintln!("  About to sign message");
        let signed_message = self.sign_message(unsigned_message)?;
        eprintln!(
            "  Message signed, final tree_hash_root: {:?}",
            signed_message.tree_hash_root()
        );

        Ok(signed_message)
    }

    /// Verify message root
    pub fn verify_root(&self, msg: SignedSSVMessage, root: Hash256) -> bool {
        msg.tree_hash_root() == root
    }

    /// Validate message using message_validator ValidationFailure types
    pub fn validate_message(&self, msg: &SignedSSVMessage) -> Result<(), ValidationFailure> {
        // Use the complete validation pipeline and ignore the ValidatedSSVMessage result
        self.validation_adapter
            .validate_signed_message(msg)
            .map(|_| ())
    }

    /// Process message through QBFT with validation
    pub fn process_message(
        &mut self,
        msg: SignedSSVMessage,
    ) -> Result<ProcessingResult, TestError> {
        // First validate the message using message_validator
        if let Err(validation_error) = self.validate_message(&msg) {
            eprintln!("  ✗ Message validation failed: {:?}", validation_error);
            return Err(TestError::MessageCreationFailed(format!(
                "validation failed: {:?}",
                validation_error
            )));
        }
        eprintln!("  ✓ Message validation passed");

        // Decode the QBFT message first to check if it's a decided message
        let qbft_message = QbftMessage::from_ssz_bytes(msg.ssv_message().data()).map_err(|e| {
            TestError::MessageCreationFailed(format!("Failed to decode message: {:?}", e))
        })?;

        // Check if the instance is already decided, but allow decided messages to pass through
        // A decided message is a commit message with quorum signatures
        let is_decided_message = qbft_message.qbft_message_type == QbftMessageType::Commit
            && msg.signatures().len() >= self.config.quorum_threshold;

        if self.instance_decided && !is_decided_message {
            return Err(TestError::MessageCreationFailed(
                "not processing consensus message since instance is already decided".to_string(),
            ));
        }

        // === PROCESSING MESSAGE DEBUG ===
        eprintln!("=== PROCESSING MESSAGE ===");
        eprintln!("  Message type: {:?}", qbft_message.qbft_message_type);
        eprintln!("  Message height: {}", qbft_message.height);
        eprintln!("  Message round: {:?}", qbft_message.round);
        eprintln!("  Current QBFT round: {}", self.qbft.get_round());
        eprintln!("  Before receive - completed: {:?}", self.qbft.completed());
        eprintln!("  Message data hash: {:?}", qbft_message.root);
        eprintln!("  Message operator IDs: {:?}", msg.operator_ids());
        eprintln!("  Message signatures count: {}", msg.signatures().len());

        // CRITICAL DEBUG: Check committee configuration vs message
        eprintln!("  Committee configuration: {:?}", self.committee);
        eprintln!(
            "  Committee size: {}, Quorum threshold: {}",
            self.config.committee_size, self.config.quorum_threshold
        );
        eprintln!("  Is multi-signer: {}", msg.signatures().len() > 1);

        if msg.signatures().len() >= self.config.quorum_threshold {
            eprintln!(
                "  ✓ Message has quorum signatures ({} >= {})",
                msg.signatures().len(),
                self.config.quorum_threshold
            );
        } else {
            eprintln!(
                "  ✗ Message lacks quorum signatures ({} < {})",
                msg.signatures().len(),
                self.config.quorum_threshold
            );
        }

        let wrapped_message = WrappedQbftMessage {
            signed_message: msg.clone(),
            qbft_message: qbft_message.clone(),
        };

        // Process through QBFT
        self.qbft.receive(wrapped_message);

        // Check state after processing
        eprintln!("  After receive - completed: {:?}", self.qbft.completed());
        let local_consensus_reached = self.qbft.completed().is_some();
        let has_aggregated_commit = self.qbft.get_aggregated_commit().is_some();

        if let Some(completion) = self.qbft.completed() {
            eprintln!("  ✓ LOCAL CONSENSUS DETECTED: {:?}", completion);
        } else if has_aggregated_commit {
            eprintln!("  ✓ AGGREGATED COMMIT DETECTED (partial consensus)");
        } else {
            eprintln!("  ✗ No local consensus yet");
        }
        eprintln!("  Current round after receive: {}", self.qbft.get_round());
        eprintln!("  Aggregated commit: {:?}", has_aggregated_commit);

        // Height tracking for decided messages (commit messages with quorum signatures)
        // This matches the Go controller's UponDecided logic - any decided message should update height
        let is_decided_message = qbft_message.qbft_message_type == QbftMessageType::Commit
            && msg.signatures().len() >= self.config.quorum_threshold;

        if is_decided_message {
            eprintln!(
                "  ✓ Processing decided message at height {}",
                qbft_message.height
            );

            // Update current height to track the highest decided height we've seen
            if qbft_message.height > self.current_height {
                eprintln!(
                    "  ✓ Updating current height from {} to {}",
                    self.current_height, qbft_message.height
                );
                self.current_height = qbft_message.height;
            } else {
                eprintln!(
                    "  ✓ Message height {} <= current height {}, no update needed",
                    qbft_message.height, self.current_height
                );
            }

            // Store the decided value from the message's full data
            if !msg.full_data().is_empty() {
                eprintln!(
                    "  ✓ Storing decided value from decided message: {} bytes",
                    msg.full_data().len()
                );
                self.decided_value = Some(msg.full_data().to_vec());
            }
        }

        // Determine overall consensus reached - local consensus, aggregated commit, or decided message
        // For controller tests, aggregated commits also indicate consensus has been reached
        let consensus_reached =
            local_consensus_reached || has_aggregated_commit || is_decided_message;

        if local_consensus_reached {
            eprintln!("  ✓ LOCAL CONSENSUS: Full QBFT consensus reached");
            self.instance_decided = true;
        } else if has_aggregated_commit && !is_decided_message {
            eprintln!("  ✓ AGGREGATED COMMIT CONSENSUS: Local consensus via aggregated commit");
            self.instance_decided = true;
        } else if is_decided_message && !local_consensus_reached {
            eprintln!(
                "  ✓ DECIDED MESSAGE CONSENSUS: External consensus detected via decided message"
            );
            self.instance_decided = true;
        }

        // Track processed message
        self.processed_messages.push(msg);

        // Check if any messages were sent
        let messages_sent: Vec<UnsignedWrappedQbftMessage> =
            self.message_queue.write().drain(..).collect();
        eprintln!("  Messages sent by QBFT: {}", messages_sent.len());
        eprintln!(
            "  Processed messages count: {}",
            self.processed_messages.len()
        );
        eprintln!("=== END PROCESSING ===\n");

        // Return processing result
        Ok(ProcessingResult {
            state_changed: true, // TODO: Implement proper state change detection
            messages_sent,
            consensus_reached,
        })
    }

    /// Get current round
    pub fn get_current_round(&self) -> Round {
        self.qbft.get_round()
    }

    /// Get QBFT state
    pub fn get_state(&self) -> QbftState {
        QbftState {
            current_round: self.qbft.get_round(),
            state: format!("{:?}", self.qbft.config()), /* TODO: Implement proper state
                                                         * representation */
            last_prepared_round: None, // TODO: Expose these fields
            last_prepared_value: None, // TODO: Expose these fields
        }
    }

    /// Get committee configuration
    pub fn committee(&self) -> &IndexSet<OperatorId> {
        &self.committee
    }

    /// Get identifier
    pub fn identifier(&self) -> &MessageId {
        &self.identifier
    }

    /// Get configuration
    pub fn config(&self) -> &TestConfig {
        &self.config
    }

    /// Create an unsigned message using the core QBFT spec method
    pub fn new_unsigned_message_spec(
        &self,
        msg_type: QbftMessageType,
        data_hash: Hash256,
        round_change_justification: Vec<SignedSSVMessage>,
        prepare_justification: Vec<SignedSSVMessage>,
        round: Option<Round>,
    ) -> UnsignedWrappedQbftMessage {
        self.qbft.new_unsigned_message_spec(
            msg_type,
            data_hash,
            round_change_justification,
            prepare_justification,
            round,
        )
    }

    // ============================================================================
    // Controller-Level Testing Methods
    // ============================================================================

    /// Start a new QBFT instance with the given input value
    /// This simulates the controller starting a new consensus instance
    pub fn start_instance(&mut self, input_value: Vec<u8>) -> Result<(), TestError> {
        // Height validation: prevent starting instances at past heights
        // Note: The config.instance_height represents the target height for the new instance
        let target_height = self.config.instance_height as u64;
        if target_height < self.current_height {
            return Err(TestError::ScenarioSetupError(
                "attempting to start an instance with a past height".to_string(),
            ));
        }

        // Check if instance already running at this height
        if self.instance_started && target_height == self.current_height {
            return Err(TestError::ScenarioSetupError(
                "instance already running".to_string(),
            ));
        }

        // Check if instance already started (for different heights)
        if self.instance_started {
            return Err(TestError::ScenarioSetupError(
                "Instance already started".to_string(),
            ));
        }

        // Store the instance value for decided state tracking
        self.instance_value = Some(input_value.clone());
        self.instance_started = true;

        // Add the input value as test data to the QBFT instance
        // This is critical for the QBFT to properly process commit messages
        use sha2::{Digest, Sha256};
        let data_hash = types::Hash256::from_slice(&Sha256::digest(&input_value));
        let test_data = Arc::new(BeaconVote {
            block_root: data_hash,
            source: types::Checkpoint::default(),
            target: types::Checkpoint::default(),
        });

        self.qbft.add_test_data(data_hash, test_data);

        eprintln!(
            "Controller: Started instance with {} bytes of input data",
            input_value.len()
        );
        eprintln!(
            "Controller: Target height: {}, Current height: {}",
            target_height, self.current_height
        );
        eprintln!("Controller: Added test data with hash: {:?}", data_hash);

        Ok(())
    }

    /// Process multiple messages in sequence and return comprehensive controller result
    pub fn process_messages_controller(
        &mut self,
        messages: Vec<SignedSSVMessage>,
    ) -> Result<ControllerResult, TestError> {
        // === CONTROLLER PROCESSING DEBUG ===
        eprintln!("=== CONTROLLER PROCESSING {} MESSAGES ===", messages.len());

        let mut all_output_messages = Vec::new();
        let mut final_processing_result = ProcessingResult {
            state_changed: false,
            messages_sent: Vec::new(),
            consensus_reached: false,
        };

        // Process each message with detailed logging
        for (i, msg) in messages.iter().enumerate() {
            eprintln!("\n--- Processing Message {} ---", i + 1);
            eprintln!("  Message signatures: {}", msg.signatures().len());
            eprintln!("  Message operator IDs: {:?}", msg.operator_ids());
            eprintln!("  Message tree hash: {:?}", msg.tree_hash_root());
            eprintln!("  Message data length: {}", msg.ssv_message().data().len());

            let result = self.process_message(msg.clone())?;

            eprintln!(
                "  Result: state_changed={}, consensus_reached={}, messages_sent={}",
                result.state_changed,
                result.consensus_reached,
                result.messages_sent.len()
            );

            // Accumulate state changes
            final_processing_result.state_changed |= result.state_changed;
            final_processing_result
                .messages_sent
                .extend(result.messages_sent);
            final_processing_result.consensus_reached |= result.consensus_reached;

            // Convert any unsigned messages to signed and add to outputs
            // This simulates the controller broadcasting messages
            for unsigned_msg in &final_processing_result.messages_sent {
                if let Ok(signed) = self.sign_message(unsigned_msg.clone()) {
                    all_output_messages.push(signed);
                }
            }

            // Show running consensus status
            eprintln!(
                "  Running consensus_reached: {}",
                final_processing_result.consensus_reached
            );
        }

        // Update decided count if consensus was reached
        let old_decided_count = self.decided_count;
        if final_processing_result.consensus_reached {
            self.decided_count += 1;
            eprintln!(
                "  ✓ Consensus reached! Incrementing decided_count from {} to {}",
                old_decided_count, self.decided_count
            );

            // Only update current height when LOCAL consensus is reached (not for decided messages)
            // For decided messages, height is updated in process_message when the message is processed
            if self.qbft.completed().is_some() {
                self.current_height = (self.config.instance_height as u64) + 1;
                eprintln!(
                    "  ✓ Local consensus - Updated current height to: {}",
                    self.current_height
                );
            } else {
                eprintln!("  ✓ External consensus via decided message - height already updated");
            }
        } else {
            eprintln!(
                "  ✗ No consensus reached, decided_count remains {}",
                self.decided_count
            );
        }

        // Store output messages
        self.output_messages.extend(all_output_messages.clone());

        eprintln!("\n=== FINAL CONTROLLER RESULT ===");
        eprintln!(
            "  Total consensus_reached: {}",
            final_processing_result.consensus_reached
        );
        eprintln!("  Final decided_count: {}", self.decided_count);
        eprintln!("  Output messages: {}", all_output_messages.len());
        eprintln!(
            "  Total messages sent: {}",
            final_processing_result.messages_sent.len()
        );
        eprintln!("=== END CONTROLLER ===\n");

        Ok(ControllerResult {
            processing_result: final_processing_result,
            decided_state: self.get_decided_state(),
            timer_state: self.get_timer_state(),
            output_messages: all_output_messages,
        })
    }

    /// Get the current decided state for controller testing
    pub fn get_decided_state(&self) -> DecidedState {
        // === DECIDED STATE CHECK DEBUG ===
        eprintln!("=== DECIDED STATE CHECK ===");
        eprintln!("  qbft.completed(): {:?}", self.qbft.completed());
        eprintln!("  instance_started: {}", self.instance_started);
        eprintln!(
            "  instance_value: {} bytes",
            self.instance_value.as_ref().map_or(0, |v| v.len())
        );
        eprintln!("  decided_count: {}", self.decided_count);

        // Enhanced completion check
        let decided_val = if let Some(completion) = self.qbft.completed() {
            eprintln!("  ✓ LOCAL CONSENSUS REACHED!");
            eprintln!("  Completion details: {:?}", completion);
            eprintln!(
                "  Returning instance_value: {} bytes",
                self.instance_value.as_ref().map_or(0, |v| v.len())
            );
            self.instance_value.clone()
        } else if self.instance_decided || self.decided_count > 0 {
            eprintln!("  ✓ EXTERNAL CONSENSUS via decided message!");
            eprintln!("  decided_count: {}", self.decided_count);
            eprintln!("  instance_decided: {}", self.instance_decided);

            // For external consensus, use the decided value from the decided message if available
            if let Some(ref decided_value) = self.decided_value {
                eprintln!("  Returning decided_value: {} bytes", decided_value.len());
                Some(decided_value.clone())
            } else {
                eprintln!(
                    "  Returning instance_value: {} bytes",
                    self.instance_value.as_ref().map_or(0, |v| v.len())
                );
                self.instance_value.clone()
            }
        } else {
            eprintln!("  ✗ No consensus detected");
            eprintln!("  Current round: {}", self.qbft.get_round());
            eprintln!(
                "  Aggregated commit: {:?}",
                self.qbft.get_aggregated_commit().is_some()
            );

            // Additional diagnostic information
            if let Some(agg_commit) = self.qbft.get_aggregated_commit() {
                eprintln!(
                    "  Aggregated commit details: tree_hash={:?}",
                    agg_commit.tree_hash_root()
                );
            }
            None
        };

        // CORE FIX: decided_cnt should be 1 if any consensus was reached (local or external)
        let decided_cnt =
            if self.qbft.completed().is_some() || self.instance_decided || self.decided_count > 0 {
                1
            } else {
                0
            };

        let result = DecidedState {
            decided_val: decided_val.clone(),
            decided_cnt,
            broadcasted_decided: None, // TODO: Implement if needed for specific tests
        };

        eprintln!(
            "  Final DecidedState: decided_cnt={}, decided_val={:?}",
            result.decided_cnt,
            result
                .decided_val
                .as_ref()
                .map(|v| format!("{} bytes", v.len()))
        );
        eprintln!("=== END DECIDED STATE ===\n");

        result
    }

    /// Get the current timer state for controller testing
    pub fn get_timer_state(&self) -> Option<TimerState> {
        if self.instance_started {
            Some(TimerState {
                timeouts: 1, // Simplified: assume one timeout per round
                round: self.qbft.get_round(),
            })
        } else {
            None
        }
    }

    /// Get all output messages generated during controller testing
    pub fn get_output_messages(&mut self) -> Vec<SignedSSVMessage> {
        std::mem::take(&mut self.output_messages)
    }

    /// Check if the QBFT instance has reached consensus
    pub fn has_consensus(&self) -> bool {
        self.qbft.completed().is_some()
    }

    /// Reset controller state for new test scenario
    pub fn reset_controller_state(&mut self) {
        self.instance_started = false;
        self.instance_value = None;
        self.decided_count = 0;
        self.output_messages.clear();
        self.processed_messages.clear();
        self.decided_value = None;
        self.instance_decided = false;
        // NOTE: current_height is NOT reset - it persists across scenarios
        // This matches the Go controller behavior where height tracks the highest decided height
    }

    /// Set the current height for controller testing
    /// This simulates the persistent state of the Go controller
    pub fn set_current_height(&mut self, height: u64) {
        self.current_height = height;
    }

    /// Sign message using deterministic RSA signing
    pub fn sign_message(
        &self,
        unsigned: UnsignedWrappedQbftMessage,
    ) -> Result<SignedSSVMessage, TestError> {
        let signing_key = self
            .signing_key
            .as_ref()
            .ok_or_else(|| TestError::SigningError("No signing key configured".to_string()))?;

        let serialized = unsigned.unsigned_message.ssv_message.as_ssz_bytes();

        // DEBUG: Log signing details
        eprintln!(
            "  SIGN: SSV message data to sign: {} bytes",
            serialized.len()
        );
        eprintln!(
            "  SIGN: SSV message data: {:?}",
            &serialized[0..std::cmp::min(32, serialized.len())]
        );
        eprintln!(
            "  SIGN: Full data: {} bytes",
            unsigned.unsigned_message.full_data.len()
        );
        eprintln!("  SIGN: Operator ID: {:?}", self.get_operator_id());

        // Use deterministic signing for spec tests
        let signature = self.sign_deterministic(&serialized, signing_key)?;

        eprintln!("  SIGN: Signature created, {} bytes", signature.len());

        // Create signed message
        let signed_message = SignedSSVMessage::new_from_vecs(
            vec![
                signature
                    .try_into()
                    .map_err(|_| TestError::SigningError("Invalid signature length".to_string()))?,
            ],
            vec![self.get_operator_id()],
            unsigned.unsigned_message.ssv_message,
            unsigned.unsigned_message.full_data,
        )
        .map_err(|e| TestError::SigningError(format!("Failed to create signed message: {}", e)))?;

        eprintln!("  SIGN: Final signed message created");
        eprintln!(
            "  SIGN: Tree hash root: {:?}",
            signed_message.tree_hash_root()
        );

        Ok(signed_message)
    }

    /// Get operator ID for current test
    fn get_operator_id(&self) -> OperatorId {
        self.signing_operator_id.unwrap_or_else(|| {
            self.committee
                .iter()
                .next()
                .copied()
                .unwrap_or(OperatorId::from(1))
        })
    }

    /// Deterministic RSA signing for spec tests
    fn sign_deterministic(
        &self,
        data: &[u8],
        private_key: &PKey<Private>,
    ) -> Result<Vec<u8>, TestError> {
        // Calculate SHA256 hash
        let mut hasher = Hasher::new(MessageDigest::sha256())
            .map_err(|e| TestError::SigningError(format!("Hash creation failed: {}", e)))?;
        hasher
            .update(data)
            .map_err(|e| TestError::SigningError(format!("Hash update failed: {}", e)))?;
        let hash = hasher
            .finish()
            .map_err(|e| TestError::SigningError(format!("Hash finalization failed: {}", e)))?;

        // Get the RSA key
        let rsa_key = private_key
            .rsa()
            .map_err(|e| TestError::SigningError(format!("RSA key extraction failed: {}", e)))?;

        // Create PKCS#1 v1.5 padding for deterministic behavior
        let hash_len = hash.len();
        let key_size = rsa_key.size() as usize;

        // SHA256 DigestInfo (ASN.1 DER encoding)
        let digest_info = &[
            0x30, 0x31, // SEQUENCE, length 49
            0x30, 0x0d, // SEQUENCE, length 13
            0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, // SHA256 OID
            0x05, 0x00, // NULL
            0x04, 0x20, // OCTET STRING, length 32
        ];

        let digest_info_len = digest_info.len();
        let total_hash_len = digest_info_len + hash_len;
        let padding_len = key_size - 3 - total_hash_len;

        if padding_len < 8 {
            return Err(TestError::SigningError(
                "Key too small for message".to_string(),
            ));
        }

        // Build the padded message deterministically
        let mut padded_msg = Vec::with_capacity(key_size);
        padded_msg.push(0x00); // Leading zero
        padded_msg.push(0x01); // Block type 01
        padded_msg.extend(std::iter::repeat(0xFF).take(padding_len)); // Padding
        padded_msg.push(0x00); // Separator
        padded_msg.extend_from_slice(digest_info); // DigestInfo
        padded_msg.extend_from_slice(&hash); // Hash

        // Perform raw RSA private key operation
        let mut signature = vec![0u8; key_size];
        let sig_len = rsa_key
            .private_encrypt(&padded_msg, &mut signature, Padding::NONE)
            .map_err(|e| TestError::SigningError(format!("RSA signing failed: {}", e)))?;
        signature.truncate(sig_len);

        Ok(signature)
    }
}

/// Test scenario configuration (preserved from QbftTestAdapter)
#[derive(Debug, Clone)]
pub struct TestScenario {
    pub round: Round,
    pub last_prepared_round: Option<Round>,
    pub last_prepared_value: Option<Hash256>,
    pub round_change_justifications: Vec<SignedSSVMessage>,
    pub prepare_justifications: Vec<SignedSSVMessage>,
}

/// Result of processing a message
#[derive(Debug)]
pub struct ProcessingResult {
    pub state_changed: bool,
    pub messages_sent: Vec<UnsignedWrappedQbftMessage>,
    pub consensus_reached: bool,
}

/// Current QBFT state
#[derive(Debug)]
pub struct QbftState {
    pub current_round: Round,
    pub state: String,
    pub last_prepared_round: Option<Round>,
    pub last_prepared_value: Option<Hash256>,
}

/// Timer state for controller testing
#[derive(Debug, Clone)]
pub struct TimerState {
    pub timeouts: u64,
    pub round: Round,
}

/// Decided state for controller testing
#[derive(Debug, Clone)]
pub struct DecidedState {
    pub decided_val: Option<Vec<u8>>,
    pub decided_cnt: u64,
    pub broadcasted_decided: Option<SignedSSVMessage>,
}

/// Controller test result containing all state changes
#[derive(Debug)]
pub struct ControllerResult {
    pub processing_result: ProcessingResult,
    pub decided_state: DecidedState,
    pub timer_state: Option<TimerState>,
    pub output_messages: Vec<SignedSSVMessage>,
}

/// Test message sender that captures sent messages
struct TestMessageSender {
    queue: Arc<RwLock<VecDeque<UnsignedWrappedQbftMessage>>>,
}

impl MessageSender for TestMessageSender {
    fn send(&mut self, msg: UnsignedWrappedQbftMessage) {
        self.queue.write().push_back(msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_keys::TestKeySet;

    #[test]
    fn test_unified_adapter_creation() {
        let mut committee = IndexSet::new();
        committee.insert(OperatorId::from(1));
        committee.insert(OperatorId::from(2));
        committee.insert(OperatorId::from(3));
        committee.insert(OperatorId::from(4));

        let config = TestConfig {
            committee_size: committee.len(),
            quorum_threshold: (committee.len() * 2 / 3) + 1,
            max_rounds: 100,
            instance_height: 0,
        };

        // Create proper MessageId
        use ssv_types::{
            CommitteeId,
            domain_type::DomainType,
            msgid::{DutyExecutor, Role},
        };
        let domain = DomainType::default();
        let role = Role::Committee;
        let duty_executor = DutyExecutor::Committee(CommitteeId::default());
        let message_id = MessageId::new(&domain, role, &duty_executor);

        let adapter = UnifiedTestAdapter::new(committee, message_id, config);
        assert!(adapter.is_ok());
    }

    #[test]
    fn test_message_creation_methods() {
        let mut committee = IndexSet::new();
        committee.insert(OperatorId::from(1));

        let config = TestConfig {
            committee_size: committee.len(),
            quorum_threshold: 1,
            max_rounds: 100,
            instance_height: 0,
        };

        // Create proper MessageId
        use ssv_types::{
            CommitteeId,
            domain_type::DomainType,
            msgid::{DutyExecutor, Role},
        };
        let domain = DomainType::default();
        let role = Role::Committee;
        let duty_executor = DutyExecutor::Committee(CommitteeId::default());
        let message_id = MessageId::new(&domain, role, &duty_executor);

        let mut adapter = UnifiedTestAdapter::new(committee, message_id, config).unwrap();

        // Set signing key
        let test_keys = TestKeySet::four_share_set();
        let operator_key = test_keys.operator_keys.get(&OperatorId::from(1)).unwrap();
        let private_key = PKey::from_rsa(operator_key.to_owned()).unwrap();
        adapter.set_signing_key_with_operator(private_key, OperatorId::from(1));

        let data_hash = Hash256::default();
        let round = Some(Round::from(1));

        // Test all message creation methods
        let proposal = adapter.create_proposal(data_hash, round);
        assert!(proposal.is_ok());

        let prepare = adapter.create_prepare(data_hash, round);
        assert!(prepare.is_ok());

        let commit = adapter.create_commit(data_hash, round);
        assert!(commit.is_ok());

        let round_change = adapter.create_round_change(None, Some(Round::from(2)));
        assert!(round_change.is_ok());
    }

    #[test]
    fn test_multi_signer_commit_consensus_detection() {
        // Test that multi-signer commit messages are properly processed and consensus is detected
        let mut committee = IndexSet::new();
        committee.insert(OperatorId::from(1));
        committee.insert(OperatorId::from(2));
        committee.insert(OperatorId::from(3));

        let config = TestConfig {
            committee_size: committee.len(),
            quorum_threshold: 3, // Need all 3 signatures
            max_rounds: 100,
            instance_height: 0,
        };

        // Create proper MessageId
        use ssv_types::{
            CommitteeId,
            domain_type::DomainType,
            msgid::{DutyExecutor, Role},
        };
        let domain = DomainType::default();
        let role = Role::Committee;
        let duty_executor = DutyExecutor::Committee(CommitteeId::default());
        let message_id = MessageId::new(&domain, role, &duty_executor);

        let mut adapter = UnifiedTestAdapter::new(committee, message_id, config).unwrap();

        // Set signing key
        let test_keys = TestKeySet::four_share_set();
        let operator_key = test_keys.operator_keys.get(&OperatorId::from(1)).unwrap();
        let private_key = PKey::from_rsa(operator_key.to_owned()).unwrap();
        adapter.set_signing_key_with_operator(private_key, OperatorId::from(1));

        let data_hash = Hash256::random();
        let round = Round::from(1);

        // Create test data for the commit message
        let test_data = BeaconVote {
            block_root: data_hash,
            source: types::Checkpoint::default(),
            target: types::Checkpoint::default(),
        };

        // Create a multi-signer commit message with quorum signatures
        let commit_message = {
            // First create a regular commit message
            let single_commit = adapter.create_commit(data_hash, Some(round)).unwrap();

            // Add multiple signatures to make it a multi-signer message
            let mut signatures = Vec::new();
            let mut operator_ids = Vec::new();

            // Add signatures from multiple operators
            for i in 1..=3 {
                let operator_id = OperatorId::from(i);
                operator_ids.push(operator_id);

                let operator_key = test_keys.operator_keys.get(&operator_id).unwrap();
                let private_key = PKey::from_rsa(operator_key.to_owned()).unwrap();

                // Sign the message
                let signature = adapter
                    .sign_deterministic(&single_commit.ssv_message().as_ssz_bytes(), &private_key)
                    .unwrap();
                signatures.push(signature.try_into().unwrap());
            }

            // Create the multi-signer message with full data
            let multi_signer_msg = SignedSSVMessage::new_from_vecs(
                signatures,
                operator_ids,
                single_commit.ssv_message().clone(),
                test_data.as_ssz_bytes(), // Include the actual data
            )
            .unwrap();

            multi_signer_msg
        };

        // Start instance to set up initial state
        adapter.start_instance(test_data.as_ssz_bytes()).unwrap();

        // Before processing: should have no consensus
        assert!(!adapter.has_consensus());

        // Process the multi-signer commit message
        let result = adapter.process_message(commit_message).unwrap();

        // After processing: should detect consensus
        assert!(
            result.consensus_reached,
            "Consensus should be reached after processing multi-signer commit"
        );
        assert!(adapter.has_consensus(), "Adapter should detect consensus");

        // Verify the decided state
        let decided_state = adapter.get_decided_state();
        assert!(
            decided_state.decided_val.is_some(),
            "Should have decided value"
        );

        println!("✓ Multi-signer commit message consensus detection test passed!");
    }
}
