use serde::Deserialize;
use ssv_types::message::SignedSSVMessage;

use super::adapter::{QbftTestAdapter, TestContext, TestType};
use crate::{QbftSpecTestType, SpecTest, SpecTestType, types::TestSignedSSVMessage};

impl SpecTest for QbftMessageTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self) -> bool {
        // Create test context
        let test_context = TestContext::new(self.name.clone(), TestType::QbftMessage)
            .with_expected_errors(self.expected_errors());

        // Create adapter for validation testing with default committee
        let mut adapter = match QbftTestAdapter::with_default_committee() {
            Ok(adapter) => adapter.with_test_context(test_context),
            Err(e) => {
                eprintln!("Failed to create adapter: {}", e);
                return false;
            }
        };

        // Try to create the signed message
        let scenario_result = match self.create_signed_message() {
            Ok(message) => {
                // Execute validation scenario with the message
                adapter.execute_validation_scenario(message)
            }
            Err(creation_error) => {
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

        // Assert validation result
        self.assert_validation_result(&scenario_result)
    }

    fn setup(&mut self) {
        // No setup needed for message validation tests
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
        use base64::prelude::*;
        use ssv_types::message::SignedSSVMessage;

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
        
        // Check for zero signers first (before sorting) to match Go validation order
        if operator_ids.iter().any(|&id| id == ssv_types::OperatorId(0)) {
            return Err("signer ID 0 not allowed".to_string());
        }
        
        operator_ids.sort();
        
        // Create our SignedSSVMessage
        SignedSSVMessage::new_from_vecs(
            signatures,
            operator_ids,
            ssv_message,
            Vec::new(),
        )
        .map_err(|e| self.map_signed_ssv_error_to_go_string(&e))
    }

    /// Map SignedSSVMessage errors to Go error strings
    fn map_signed_ssv_error_to_go_string(
        &self,
        error: &ssv_types::message::SignedSSVMessageError,
    ) -> String {
        use ssv_types::message::SignedSSVMessageError;
        match error {
            SignedSSVMessageError::NoSigners => "no signers".to_string(),
            SignedSSVMessageError::ZeroSigner => "signer ID 0 not allowed".to_string(),
            SignedSSVMessageError::DuplicatedSigner => "non unique signer".to_string(),
            SignedSSVMessageError::SignersAndSignaturesWithDifferentLength => {
                "number of signatures is different than number of signers".to_string()
            }
            SignedSSVMessageError::NoSignatures => "no signatures".to_string(),
            SignedSSVMessageError::SignersNotSorted => "signers not sorted".to_string(),
            SignedSSVMessageError::TooManySignatures { provided, max } => {
                format!("too many signatures: provided {}, maximum allowed is {}", provided, max)
            }
            SignedSSVMessageError::TooManyOperatorIDs { provided, max } => {
                format!("too many operator IDs: provided {}, maximum allowed is {}", provided, max)
            }
            SignedSSVMessageError::FullDataTooLong { provided, max } => {
                format!("full data is too long: {} bytes, maximum allowed is {} bytes", provided, max)
            }
            SignedSSVMessageError::SSVMessageError(ssv_error) => {
                format!("SSV message error: {:?}", ssv_error)
            }
        }
    }

    /// Get expected errors from test
    fn expected_errors(&self) -> Vec<String> {
        vec![self.expected_error.clone()]
    }

    /// Assert validation result
    fn assert_validation_result(&self, result: &super::adapter::ScenarioResult) -> bool {
        // Check for expected errors first
        if !self.expected_error.is_empty() {
            if result
                .go_formatted_errors
                .iter()
                .any(|err| err.contains(&self.expected_error))
            {
                eprintln!("✓ Expected validation error found: {}", self.expected_error);
                return true;
            } else {
                eprintln!(
                    "✗ Expected error '{}' not found in: {:?}",
                    self.expected_error, result.go_formatted_errors
                );
                return false;
            }
        }

        // If no errors expected, validation should pass
        if !result.processing_result.validation_result.is_valid {
            eprintln!(
                "✗ Message validation failed: {:?}",
                result.processing_result.validation_result.errors
            );
            return false;
        }

        // Check if any validation errors were found when none expected
        if !result.validation_errors.is_empty() {
            eprintln!(
                "✗ Unexpected validation errors: {:?}",
                result.validation_errors
            );
            return false;
        }

        eprintln!("✓ Message validation passed as expected");
        true
    }
}

#[derive(Debug, Clone, Deserialize)]
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
}
