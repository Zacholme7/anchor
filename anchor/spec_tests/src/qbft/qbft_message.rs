use super::adapter::{QbftTestAdapter, TestContext, TestType};
use crate::qbft::adapter::error_mapping::map_signed_ssv_error_to_go_format;
use crate::{QbftSpecTestType, SpecTest, SpecTestType, types::TestSignedSSVMessage};
use base64::prelude::*;
use serde::Deserialize;
use ssv_types::message::SignedSSVMessage;

#[derive(Deserialize)]
pub struct QbftMessageTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(rename = "Messages")]
    pub messages: Vec<TestSignedSSVMessage>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,

    // Setup state fields - not from input JSON
    #[serde(skip)]
    pub test_context: Option<TestContext>,
    #[serde(skip)]
    pub adapter: Option<Box<QbftTestAdapter>>,
    #[serde(skip)]
    pub setup_error: Option<String>,
}

impl SpecTest for QbftMessageTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) {
        // Create test context and qbft adapter
        let test_context = TestContext::new(self.name.clone(), TestType::QbftMessage)
            .with_expected_errors(vec![self.expected_error.clone()]);
        let adapter = match QbftTestAdapter::with_default_committee() {
            Ok(adapter) => adapter.with_test_context(test_context.clone()),
            Err(e) => {
                self.setup_error = Some(format!("Failed to create adapter: {}", e));
                return;
            }
        };

        // Store successful setup
        self.test_context = Some(test_context);
        self.adapter = Some(Box::new(adapter));
    }

    fn run(&self) -> bool {
        if !(self.setup_error.is_none() && self.test_context.is_some() && self.adapter.is_some()) {
            return false;
        }

        // Get pre-created adapter
        let adapter = self.adapter.as_ref().unwrap().as_ref();

        // Try to create the signed message
        let scenario_result = match self.create_signed_message() {
            Ok(message) => {
                // Execute validation scenario with the message
                adapter.execute_validation_scenario(message)
            }
            Err(creation_error) => {
                println!("{:?}", creation_error);
                // Check if this creation error matches the expected error
                if !self.expected_error.is_empty() && creation_error.contains(&self.expected_error)
                {
                    eprintln!("✓ Expected creation error found: {}", creation_error);
                    return true;
                } else {
                    eprintln!("✗ Unexpected creation error: {}", creation_error);
                    return false;
                }
            }
        };

        // Check for expected errors first
        if !self.expected_error.is_empty() {
            if scenario_result
                .go_formatted_errors
                .iter()
                .any(|err| err.contains(&self.expected_error))
            {
                return true;
            } else {
                return false;
            }
        }

        // If no errors expected, validation should pass
        if !scenario_result.processing_result.validation_result.is_valid {
            return false;
        }

        // Check if any validation errors were found when none expected
        if !scenario_result.validation_errors.is_empty() {
            return false;
        }

        true
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::QbftMessage)
    }
}

impl QbftMessageTest {
    /// Create signed message from test data, handling errors properly
    fn create_signed_message(&self) -> Result<SignedSSVMessage, String> {
        // Take first message from messages array for testing
        let test_msg = self
            .messages
            .get(0)
            .unwrap_or_else(|| panic!("No messages in test data for {}", self.name));

        // Convert TestSignedSSVMessage to SignedSSVMessage
        self.convert_test_message(test_msg)
    }

    /// Convert test message to SignedSSVMessage, returning errors to be mapped
    fn convert_test_message(
        &self,
        test_msg: &TestSignedSSVMessage,
    ) -> Result<SignedSSVMessage, String> {
        // Handle null SSVMessage case
        let ssv_message = match &test_msg.ssv_message {
            Some(msg) => msg.clone(),
            None => {
                return Err("nil SSVMessage".to_string());
            }
        };

        // Convert base64 signatures to bytes
        let mut signatures = Vec::new();
        for sig_str in &test_msg.signatures {
            let sig_bytes = BASE64_STANDARD
                .decode(sig_str.as_bytes())
                .map_err(|_| "failed to decode base64 signature".to_string())?;

            // Pad or truncate signature to 256 bytes for RSA signature format
            let mut sig_array = [0u8; 256];
            if sig_bytes.len() <= 256 {
                sig_array[..sig_bytes.len()].copy_from_slice(&sig_bytes);
            } else {
                sig_array.copy_from_slice(&sig_bytes[..256]);
            }
            signatures.push(sig_array);
        }

        // Get operator IDs and sort them before creating the message
        // The Go tests expect sorting to happen before zero validation
        let mut operator_ids = test_msg.operator_ids.clone().unwrap_or_default();
        operator_ids.sort();

        // Decode full_data from base64 string to bytes
        let full_data_bytes = match &test_msg.full_data {
            Some(base64_str) => BASE64_STANDARD
                .decode(base64_str.as_bytes())
                .map_err(|e| format!("failed to decode base64 full_data: {}", e))?,
            None => Vec::new(),
        };

        // Create our SignedSSVMessage
        SignedSSVMessage::new_from_vecs(signatures, operator_ids, ssv_message, full_data_bytes)
            .map_err(|e| map_signed_ssv_error_to_go_format(&e))
    }
}
