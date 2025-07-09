use crate::{
    Config, ConfigBuilder, DefaultLeaderFunction, InstanceHeight, MessageSender, Qbft, TestConfig, TestError,
    UnsignedWrappedQbftMessage, WrappedQbftMessage,
};
use openssl::{
    pkey::{PKey, Private},
};
use parking_lot::RwLock;
use ssv_types::{
    consensus::{BeaconVote, QbftMessage},
    message::SignedSSVMessage,
    msgid::MessageId,
    IndexSet, OperatorId, Round,
};
use ssz::{Decode, Encode};
use std::{collections::VecDeque, sync::Arc};
use types::Hash256;

/// Test adapter that provides a clean interface for all spec test categories
pub struct QbftTestAdapter {
    qbft: Qbft<DefaultLeaderFunction, BeaconVote, TestMessageSender>,
    message_queue: Arc<RwLock<VecDeque<UnsignedWrappedQbftMessage>>>,
    signing_key: Option<PKey<Private>>,
}

impl QbftTestAdapter {
    /// Create a new test adapter with the given committee and configuration
    pub fn new(
        committee: IndexSet<OperatorId>,
        identifier: MessageId,
        config: TestConfig,
    ) -> Result<Self, TestError> {
        let qbft_config: Config<DefaultLeaderFunction> = ConfigBuilder::new(
            OperatorId::from(1),
            InstanceHeight::from(config.instance_height as usize),
            committee,
        )
        .build()
        .map_err(|e| TestError::ScenarioSetupError(format!("Config build failed: {}", e)))?;

        // Create test data for QBFT
        let test_data = BeaconVote {
            block_root: Hash256::random(),
            source: types::Checkpoint::default(),
            target: types::Checkpoint::default(),
        };

        let message_queue = Arc::new(RwLock::new(VecDeque::new()));
        let message_sender = TestMessageSender {
            queue: message_queue.clone(),
        };

        let qbft = Qbft::new(qbft_config, test_data, identifier, message_sender);

        Ok(Self {
            qbft,
            message_queue,
            signing_key: None,
        })
    }

    /// Set the signing key for message signing
    pub fn set_signing_key(&mut self, key: PKey<Private>) {
        self.signing_key = Some(key);
    }

    /// Create a proposal message using core QBFT logic
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
        let unsigned_message = self.qbft.create_proposal(test_data, round)
            .map_err(|e| TestError::MessageCreationFailed(e.to_string()))?;

        // Sign the message
        self.sign_message(unsigned_message)
    }

    /// Create a prepare message using core QBFT logic
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
        let unsigned_message = self.qbft.create_prepare(data_hash, round)
            .map_err(|e| TestError::MessageCreationFailed(e.to_string()))?;

        // Sign the message
        self.sign_message(unsigned_message)
    }

    /// Create a commit message using core QBFT logic
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
        let unsigned_message = self.qbft.create_commit(data_hash, round)
            .map_err(|e| TestError::MessageCreationFailed(e.to_string()))?;

        // Sign the message
        self.sign_message(unsigned_message)
    }

    /// Create a round change message using core QBFT logic
    pub fn create_round_change(
        &mut self,
        state_value: Option<Vec<u8>>,
        target_round: Option<Round>,
    ) -> Result<SignedSSVMessage, TestError> {
        // Create round change using core QBFT
        let unsigned_message = self.qbft.create_round_change(state_value, target_round)
            .map_err(|e| TestError::MessageCreationFailed(e.to_string()))?;

        // Sign the message
        self.sign_message(unsigned_message)
    }

    /// Set up a test scenario by configuring QBFT state
    pub fn setup_test_scenario(&mut self, scenario: TestScenario) -> Result<(), TestError> {
        // Set the current round
        // Note: This would require adding a method to set the current round
        
        // Set last prepared state
        self.qbft.set_test_state(scenario.last_prepared_round, scenario.last_prepared_value);

        // Add justifications to containers
        if !scenario.round_change_justifications.is_empty() {
            let wrapped_messages: Vec<WrappedQbftMessage> = scenario.round_change_justifications
                .into_iter()
                .map(|signed_msg| {
                    let qbft_message = QbftMessage::from_ssz_bytes(signed_msg.ssv_message().data())
                        .map_err(|e| TestError::JustificationError(format!("Failed to decode QBFT message: {:?}", e)))?;
                    Ok(WrappedQbftMessage {
                        signed_message: signed_msg,
                        qbft_message,
                    })
                })
                .collect::<Result<Vec<_>, TestError>>()?;
            
            self.qbft.add_test_justifications(scenario.round, wrapped_messages);
        }

        if !scenario.prepare_justifications.is_empty() {
            let wrapped_messages: Vec<WrappedQbftMessage> = scenario.prepare_justifications
                .into_iter()
                .map(|signed_msg| {
                    let qbft_message = QbftMessage::from_ssz_bytes(signed_msg.ssv_message().data())
                        .map_err(|e| TestError::JustificationError(format!("Failed to decode QBFT message: {:?}", e)))?;
                    Ok(WrappedQbftMessage {
                        signed_message: signed_msg,
                        qbft_message,
                    })
                })
                .collect::<Result<Vec<_>, TestError>>()?;
            
            self.qbft.add_test_justifications(scenario.round, wrapped_messages);
        }

        Ok(())
    }

    /// Process a message through QBFT and return the result
    pub fn process_message(&mut self, msg: SignedSSVMessage) -> Result<ProcessingResult, TestError> {
        // Decode the QBFT message
        let qbft_message = QbftMessage::from_ssz_bytes(msg.ssv_message().data())
            .map_err(|e| TestError::MessageCreationFailed(format!("Failed to decode message: {:?}", e)))?;

        let wrapped_message = WrappedQbftMessage {
            signed_message: msg,
            qbft_message,
        };

        // Process through QBFT
        self.qbft.receive(wrapped_message);

        // Check if any messages were sent
        let messages_sent = self.message_queue.write().drain(..).collect();

        // Return processing result
        Ok(ProcessingResult {
            state_changed: true, // TODO: Implement proper state change detection
            messages_sent,
            consensus_reached: self.qbft.completed().is_some(),
        })
    }

    /// Get the current round
    pub fn get_current_round(&self) -> Round {
        self.qbft.get_round()
    }

    /// Get the current QBFT state
    pub fn get_state(&self) -> QbftState {
        QbftState {
            current_round: self.qbft.get_round(),
            state: format!("{:?}", self.qbft.config()), // TODO: Implement proper state representation
            last_prepared_round: None, // TODO: Expose these fields
            last_prepared_value: None, // TODO: Expose these fields
        }
    }

    /// Create an unsigned message using the core QBFT spec method
    pub fn new_unsigned_message_spec(
        &self,
        msg_type: ssv_types::consensus::QbftMessageType,
        data_hash: types::Hash256,
        round_change_justification: Vec<ssv_types::message::SignedSSVMessage>,
        prepare_justification: Vec<ssv_types::message::SignedSSVMessage>,
        round: Option<ssv_types::Round>,
    ) -> UnsignedWrappedQbftMessage {
        self.qbft.new_unsigned_message_spec(
            msg_type,
            data_hash,
            round_change_justification,
            prepare_justification,
            round,
        )
    }

    /// Sign a message using the configured signing key
    pub fn sign_message(&self, unsigned: UnsignedWrappedQbftMessage) -> Result<SignedSSVMessage, TestError> {
        let signing_key = self.signing_key.as_ref()
            .ok_or_else(|| TestError::SigningError("No signing key configured".to_string()))?;

        let serialized = unsigned.unsigned_message.ssv_message.as_ssz_bytes();

        // Use deterministic signing for spec tests
        let signature = self.sign_deterministic(&serialized, signing_key)?;

        // Create signed message
        let signed_message = SignedSSVMessage::new_from_vecs(
            vec![signature.try_into().map_err(|_| TestError::SigningError("Invalid signature length".to_string()))?],
            vec![OperatorId::from(1)],
            unsigned.unsigned_message.ssv_message,
            unsigned.unsigned_message.full_data,
        )
        .map_err(|e| TestError::SigningError(format!("Failed to create signed message: {}", e)))?;

        Ok(signed_message)
    }

    /// Deterministic RSA signing for spec tests
    fn sign_deterministic(&self, data: &[u8], private_key: &PKey<Private>) -> Result<Vec<u8>, TestError> {
        use openssl::hash::{Hasher, MessageDigest};
        use openssl::rsa::Padding;
        
        // Calculate SHA256 hash
        let mut hasher = Hasher::new(MessageDigest::sha256())
            .map_err(|e| TestError::SigningError(format!("Hash creation failed: {}", e)))?;
        hasher.update(data)
            .map_err(|e| TestError::SigningError(format!("Hash update failed: {}", e)))?;
        let hash = hasher.finish()
            .map_err(|e| TestError::SigningError(format!("Hash finalization failed: {}", e)))?;
        
        // Get the RSA key
        let rsa_key = private_key.rsa()
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
            return Err(TestError::SigningError("Key too small for message".to_string()));
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
        let sig_len = rsa_key.private_encrypt(&padded_msg, &mut signature, Padding::NONE)
            .map_err(|e| TestError::SigningError(format!("RSA signing failed: {}", e)))?;
        signature.truncate(sig_len);
        
        Ok(signature)
    }
}

/// Test scenario configuration
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

/// Test message sender that captures sent messages
struct TestMessageSender {
    queue: Arc<RwLock<VecDeque<UnsignedWrappedQbftMessage>>>,
}

impl MessageSender for TestMessageSender {
    fn send(&mut self, msg: UnsignedWrappedQbftMessage) {
        self.queue.write().push_back(msg);
    }
}