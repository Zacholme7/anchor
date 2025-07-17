use super::AdapterError;
use ssv_types::message::SignedSSVMessageError;

/// Simple error mapper for adapter functionality
pub struct ErrorMapper;

impl ErrorMapper {
    /// Map adapter error to Go-compatible format
    pub fn map_to_go_format(error: &AdapterError) -> String {
        simple_error_message(error)
    }
}

/// Map signed SSV error to Go format to match expected test patterns
pub fn map_signed_ssv_error_to_go_format(error: &SignedSSVMessageError) -> String {
    match error {
        SignedSSVMessageError::NoSigners => "no signers".to_string(),
        SignedSSVMessageError::NoSignatures => "no signatures".to_string(),
        SignedSSVMessageError::SignersAndSignaturesWithDifferentLength => "number of signatures is different than number of signers".to_string(),
        SignedSSVMessageError::ZeroSigner => "signer ID 0 not allowed".to_string(),
        SignedSSVMessageError::DuplicatedSigner => "non unique signer".to_string(),
        SignedSSVMessageError::SignersNotSorted => "signers not sorted".to_string(),
        SignedSSVMessageError::TooManySignatures { provided, max } => {
            format!("too many signatures: provided {}, maximum allowed is {}", provided, max)
        },
        SignedSSVMessageError::TooManyOperatorIDs { provided, max } => {
            format!("too many operator IDs: provided {}, maximum allowed is {}", provided, max)
        },
        SignedSSVMessageError::FullDataTooLong { provided, max } => {
            format!("full data is too long: {} bytes, maximum allowed is {} bytes", provided, max)
        },
        SignedSSVMessageError::SSVMessageError(ssv_error) => {
            format!("SSV message error: {:?}", ssv_error)
        },
    }
}

/// Simple error message formatting for adapter errors
pub fn simple_error_message(error: &AdapterError) -> String {
    match error {
        AdapterError::MessageCreation(msg) => format!("Message creation failed: {}", msg),
        AdapterError::Validation(msg) => format!("Validation failed: {}", msg),
        AdapterError::KeyLoading(msg) => format!("Key loading failed: {}", msg),
        AdapterError::Signing(msg) => format!("Signing failed: {}", msg),
        AdapterError::Config(msg) => format!("Configuration error: {}", msg),
        AdapterError::OpenSsl(e) => format!("OpenSSL error: {}", e),
        AdapterError::InvalidState(msg) => format!("Invalid state: {}", msg),
        AdapterError::Base64Decode(e) => format!("Base64 decode error: {}", e),
    }
}
