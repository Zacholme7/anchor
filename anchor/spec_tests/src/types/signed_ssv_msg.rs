use base64::prelude::*;
use serde::Deserialize;
use ssv_types::{
    OperatorId,
    message::{SSVMessage, SignedSSVMessage},
};

// Intermediate test-specific SignedSSVMessage that can handle null SSVMessage
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestSignedSSVMessage {
    #[serde(rename = "Signatures")]
    pub signatures: Vec<String>,
    #[serde(rename = "OperatorIDs")]
    pub operator_ids: Option<Vec<OperatorId>>,
    #[serde(rename = "SSVMessage")]
    pub ssv_message: Option<SSVMessage>,
    #[serde(rename = "FullData")]
    pub full_data: Option<String>,
}

impl TryFrom<TestSignedSSVMessage> for SignedSSVMessage {
    type Error = String;

    fn try_from(test_msg: TestSignedSSVMessage) -> Result<Self, Self::Error> {
        // Convert signatures from base64 strings to [u8; 256] arrays
        let mut signatures = Vec::new();
        for sig_str in &test_msg.signatures {
            let sig_bytes = BASE64_STANDARD
                .decode(sig_str.as_bytes())
                .map_err(|e| format!("Failed to decode signature: {}", e))?;

            if sig_bytes.len() != 256 {
                return Err(format!(
                    "Invalid signature length: expected 256, got {}",
                    sig_bytes.len()
                ));
            }

            let mut sig_array = [0u8; 256];
            sig_array.copy_from_slice(&sig_bytes);
            signatures.push(sig_array);
        }

        // Get SSV message or error
        let ssv_message = test_msg
            .ssv_message
            .clone()
            .ok_or_else(|| "SSVMessage is None".to_string())?;

        // Decode full_data from base64 string to bytes
        let full_data_bytes = match &test_msg.full_data {
            Some(base64_str) => BASE64_STANDARD
                .decode(base64_str.as_bytes())
                .map_err(|e| format!("failed to decode base64 full_data: {}", e))?,
            None => Vec::new(),
        };

        // Create our SignedSSVMessage
        SignedSSVMessage::new(
            signatures,
            test_msg.operator_ids.clone().unwrap_or_default(),
            ssv_message,
            full_data_bytes,
        )
        .map_err(|e| format!("Failed to create SignedSSVMessage: {:?}", e))
    }
}
