use ssv_types::message::SignedSSVMessageError;

/// Map signed SSV error to Go format to match expected test patterns
pub fn map_signed_ssv_error_to_go_format(error: &SignedSSVMessageError) -> String {
    match error {
        SignedSSVMessageError::NoSigners => "no signers".to_string(),
        SignedSSVMessageError::NoSignatures => "no signatures".to_string(),
        SignedSSVMessageError::SignersAndSignaturesWithDifferentLength => {
            "number of signatures is different than number of signers".to_string()
        }
        SignedSSVMessageError::ZeroSigner => "signer ID 0 not allowed".to_string(),
        SignedSSVMessageError::DuplicatedSigner => "non unique signer".to_string(),
        SignedSSVMessageError::SignersNotSorted => "signers not sorted".to_string(),
        SignedSSVMessageError::TooManySignatures { provided, max } => {
            format!(
                "too many signatures: provided {}, maximum allowed is {}",
                provided, max
            )
        }
        SignedSSVMessageError::TooManyOperatorIDs { provided, max } => {
            format!(
                "too many operator IDs: provided {}, maximum allowed is {}",
                provided, max
            )
        }
        SignedSSVMessageError::FullDataTooLong { provided, max } => {
            format!(
                "full data is too long: {} bytes, maximum allowed is {} bytes",
                provided, max
            )
        }
        SignedSSVMessageError::SSVMessageError(ssv_error) => {
            format!("SSV message error: {:?}", ssv_error)
        }
    }
}
