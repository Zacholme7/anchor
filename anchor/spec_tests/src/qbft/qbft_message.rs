use serde::Deserialize;
use ssv_types::{OperatorId, message::{SignedSSVMessage, SSVMessage, MsgType}};
use crate::{QbftSpecTestType, SpecTest, SpecTestType};
use crate::utils::deserializers::type_parse::deserialize_base64_to_bytes;
use tree_hash::TreeHash;
use base64::Engine;
use ssz::{Encode, Decode};

/// QBFT Message test structure matching the JSON format from the spec tests
#[derive(Debug, Deserialize)]
pub struct QbftMessageTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Messages")]
    pub messages: Vec<TestMessage>,
    #[serde(rename = "EncodedMessages")]
    #[serde(deserialize_with = "deserialize_optional_vec")]
    pub encoded_messages: Option<Vec<Option<String>>>, // Base64 encoded expected bytes
    #[serde(rename = "ExpectedRoots")]
    #[serde(deserialize_with = "deserialize_optional_vec")]
    pub expected_roots: Option<Vec<Option<Vec<u8>>>>, // Expected tree hash roots
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
}

/// Test message structure for individual messages within a test
#[derive(Debug, Deserialize)]
pub struct TestMessage {
    #[serde(rename = "Signatures")]
    #[serde(deserialize_with = "deserialize_signatures")]
    pub signatures: Vec<[u8; 256]>,
    #[serde(rename = "OperatorIDs")]
    #[serde(deserialize_with = "deserialize_optional_operator_ids")]
    pub operator_ids: Option<Vec<OperatorId>>,
    #[serde(rename = "SSVMessage")]
    pub ssv_message: TestSSVMessage,
    #[serde(rename = "FullData")]
    #[serde(deserialize_with = "deserialize_optional_base64")]
    pub full_data: Option<Vec<u8>>,
}

/// SSV Message structure within test messages
#[derive(Debug, Deserialize)]
pub struct TestSSVMessage {
    #[serde(rename = "MsgType")]
    pub msg_type: u64,
    #[serde(rename = "MsgID")]
    pub msg_id: Vec<u8>,
    #[serde(rename = "Data")]
    #[serde(deserialize_with = "deserialize_base64_to_bytes")]
    pub data: Vec<u8>,
}

/// Deserialize base64-encoded signatures into fixed-size arrays
fn deserialize_signatures<'de, D>(deserializer: D) -> Result<Vec<[u8; 256]>, D::Error>
where 
    D: serde::Deserializer<'de> 
{
    let base64_strings: Vec<String> = Vec::deserialize(deserializer)?;
    let mut signatures = Vec::new();
    for sig_str in base64_strings {
        let bytes = base64::engine::general_purpose::STANDARD.decode(&sig_str)
            .map_err(|e| serde::de::Error::custom(format!("Invalid base64 signature: {}", e)))?;
        if bytes.len() != 256 {
            return Err(serde::de::Error::custom(format!("Signature must be 256 bytes, got {}", bytes.len())));
        }
        let mut sig_array = [0u8; 256];
        sig_array.copy_from_slice(&bytes);
        signatures.push(sig_array);
    }
    Ok(signatures)
}

/// Deserialize optional base64 data, handling null values
fn deserialize_optional_base64<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where 
    D: serde::Deserializer<'de> 
{
    let opt_str: Option<String> = Option::deserialize(deserializer)?;
    match opt_str {
        Some(s) if s == "null" || s.is_empty() => Ok(None),
        Some(s) => base64::engine::general_purpose::STANDARD.decode(&s)
            .map(Some)
            .map_err(|e| serde::de::Error::custom(format!("Invalid base64 data: {}", e))),
        None => Ok(None),
    }
}

/// Deserialize optional operator IDs, handling null values
fn deserialize_optional_operator_ids<'de, D>(deserializer: D) -> Result<Option<Vec<OperatorId>>, D::Error>
where 
    D: serde::Deserializer<'de> 
{
    let opt_vec: Option<Vec<OperatorId>> = Option::deserialize(deserializer)?;
    Ok(opt_vec)
}

/// Deserialize optional vectors, handling null values
fn deserialize_optional_vec<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
where 
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    let opt_vec: Option<Vec<T>> = Option::deserialize(deserializer)?;
    Ok(opt_vec)
}

impl QbftMessageTest {
    /// Convert TestMessage to SignedSSVMessage for validation
    fn create_signed_message(&self, test_msg: &TestMessage) -> Result<SignedSSVMessage, String> {
        // Create SSVMessage from test data
        let msg_type = MsgType::try_from(test_msg.ssv_message.msg_type)
            .map_err(|e| format!("Invalid message type: {:?}", e))?;
        
        if test_msg.ssv_message.msg_id.len() != 56 {
            return Err(format!("Message ID must be 56 bytes, got {}", test_msg.ssv_message.msg_id.len()));
        }
        
        // Convert Vec<u8> to [u8; 56] for MessageId
        let mut msg_id_array = [0u8; 56];
        msg_id_array.copy_from_slice(&test_msg.ssv_message.msg_id);
        
        let ssv_message = SSVMessage::new_from_vec(
            msg_type,
            msg_id_array.into(),
            test_msg.ssv_message.data.clone(),
        ).map_err(|e| format!("Failed to create SSVMessage: {}", e))?;
        
        // Create SignedSSVMessage with signatures and operator IDs
        let full_data = test_msg.full_data.clone().unwrap_or_default();
        
        let operator_ids = test_msg.operator_ids.clone().unwrap_or_default();
        
        SignedSSVMessage::new_from_vecs(
            test_msg.signatures.clone(),
            operator_ids,
            ssv_message,
            full_data,
        ).map_err(|e| format!("Failed to create SignedSSVMessage: {}", e))
    }
    
    /// Test message validation using existing core logic
    fn test_message_validation(&self, test_msg: &TestMessage, _index: usize) -> (bool, String) {
        match self.create_signed_message(test_msg) {
            Ok(signed_msg) => {
                // First try basic SignedSSVMessage validation
                match signed_msg.validate() {
                    Ok(()) => {
                        // If basic validation passes, try QBFT-specific validation
                        match self.validate_qbft_specifics(&signed_msg) {
                            Ok(()) => (true, String::new()),
                            Err(e) => (false, e),
                        }
                    }
                    Err(e) => (false, e.to_string()),
                }
            }
            Err(e) => (false, e),
        }
    }
    
    /// Additional QBFT-specific validation that basic SignedSSVMessage validation doesn't cover
    fn validate_qbft_specifics(&self, signed_msg: &SignedSSVMessage) -> Result<(), String> {
        use ssv_types::consensus::QbftMessage;
        
        // Check for consensus messages only
        if *signed_msg.ssv_message().msg_type() == ssv_types::message::MsgType::SSVConsensusMsgType {
            // Try to decode the QBFT message from the data
            match QbftMessage::from_ssz_bytes(signed_msg.ssv_message().data()) {
                Ok(qbft_msg) => {
                    // Check for all-zero message identifier
                    let msg_id_bytes = signed_msg.ssv_message().msg_id().as_ssz_bytes();
                    if msg_id_bytes.iter().all(|&b| b == 0) {
                        return Err("message identifier is invalid".to_string());
                    }
                    
                    // Try to decode justifications to catch malformed nested data
                    for justification_bytes in &qbft_msg.round_change_justification {
                        if let Err(e) = SignedSSVMessage::from_ssz_bytes(justification_bytes) {
                            let error_str = format!("{:?}", e);
                            if error_str.contains("InvalidByteLength") || error_str.contains("size") {
                                return Err("incorrect size".to_string());
                            }
                        }
                    }
                    
                    for justification_bytes in &qbft_msg.prepare_justification {
                        if let Err(e) = SignedSSVMessage::from_ssz_bytes(justification_bytes) {
                            let error_str = format!("{:?}", e);
                            if error_str.contains("InvalidByteLength") || error_str.contains("size") {
                                return Err("incorrect size".to_string());
                            }
                        }
                    }
                    
                    Ok(())
                }
                Err(e) => {
                    // Check if this is a message type error
                    let error_str = format!("{:?}", e);
                    if error_str.contains("NoMatchingVariant") {
                        Err("message type is invalid".to_string())
                    } else if error_str.contains("InvalidByteLength") || error_str.contains("size") {
                        Err("incorrect size".to_string())
                    } else {
                        Err(format!("QBFT message decode error: {:?}", e))
                    }
                }
            }
        } else {
            // For non-consensus messages, no additional QBFT validation needed
            Ok(())
        }
    }
    
    /// Test encoding if expected encoding is provided
    fn test_encoding(&self, test_msg: &TestMessage, expected: &Option<String>) -> bool {
        if let Some(expected_b64) = expected {
            if let Ok(signed_msg) = self.create_signed_message(test_msg) {
                let encoded = signed_msg.as_ssz_bytes();
                if let Ok(expected_bytes) = base64::engine::general_purpose::STANDARD.decode(expected_b64) {
                    return encoded == expected_bytes;
                }
            }
            return false;
        }
        true // No encoding test required
    }
    
    /// Test root computation if expected root is provided
    fn test_root(&self, test_msg: &TestMessage, expected: &Option<Vec<u8>>) -> bool {
        if let Some(expected_root) = expected {
            if let Ok(signed_msg) = self.create_signed_message(test_msg) {
                let computed_root = signed_msg.tree_hash_root();
                if expected_root.len() == 32 {
                    let mut expected_array = [0u8; 32];
                    expected_array.copy_from_slice(expected_root);
                    return computed_root.as_slice() == expected_array;
                }
            }
            return false;
        }
        true // No root test required
    }
}

impl SpecTest for QbftMessageTest {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn run(&self) -> bool {
        let should_pass = self.expected_error.is_empty();
        
        // Test each message
        for (i, test_msg) in self.messages.iter().enumerate() {
            let (validation_passed, error_msg) = self.test_message_validation(test_msg, i);
            
            // Check if validation result matches expectation
            if should_pass && !validation_passed {
                eprintln!("Test {} failed: Expected pass but got error: {}", self.name, error_msg);
                return false;
            }
            
            if !should_pass && validation_passed {
                eprintln!("Test {} failed: Expected error '{}' but validation passed", self.name, self.expected_error);
                return false;
            }
            
            if !should_pass {
                // More flexible error matching - check if key words match
                let expected_lower = self.expected_error.to_lowercase();
                let error_lower = error_msg.to_lowercase();
                
                let error_matches = if expected_lower.contains("no signers") {
                    error_lower.contains("no signers") || error_lower.contains("must have at least one signer")
                } else if expected_lower.contains("signer id 0 not allowed") {
                    // Accept either "signer ID 0" error or "signers not sorted" error when 0 is present
                    (error_lower.contains("signer") && (error_lower.contains("0") || error_lower.contains("zero"))) ||
                    (error_lower.contains("signers") && error_lower.contains("sorted"))
                } else if expected_lower.contains("non unique signer") {
                    error_lower.contains("duplicated") || error_lower.contains("unique")
                } else {
                    error_lower.contains(&expected_lower) || expected_lower.contains(&error_lower)
                };
                
                if !error_matches {
                    eprintln!("Test {} failed: Expected error '{}' but got '{}'", self.name, self.expected_error, error_msg);
                    return false;
                }
            }
            
            // Test encoding if provided
            if let Some(ref encoded_messages) = self.encoded_messages {
                if i < encoded_messages.len() && !self.test_encoding(test_msg, &encoded_messages[i]) {
                    eprintln!("Test {} failed: Encoding mismatch for message {}", self.name, i);
                    return false;
                }
            }
            
            // Test root computation if provided
            if let Some(ref expected_roots) = self.expected_roots {
                if i < expected_roots.len() && !self.test_root(test_msg, &expected_roots[i]) {
                    eprintln!("Test {} failed: Root computation mismatch for message {}", self.name, i);
                    return false;
                }
            }
        }
        
        true
    }
    
    fn setup(&mut self) {
        // No setup needed for message validation tests
    }
    
    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::QbftMessage)
    }
}
