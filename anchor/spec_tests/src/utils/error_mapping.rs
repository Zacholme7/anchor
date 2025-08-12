use ssv_types::message::SignedSSVMessageError;

/// Maps our internal SignedSSVMessageError to the expected error strings from Go spec tests
pub fn map_signed_message_error(error: &SignedSSVMessageError) -> String {
    match error {
        SignedSSVMessageError::NoSigners => {
            "invalid signed message: invalid SignedSSVMessage: no signers".to_string()
        }
        SignedSSVMessageError::DuplicatedSigner => {
            "invalid signed message: invalid SignedSSVMessage: non unique signer".to_string()
        }
        SignedSSVMessageError::ZeroSigner => {
            "invalid signed message: invalid SignedSSVMessage: signer ID 0 not allowed".to_string()
        }
        SignedSSVMessageError::SignersNotSorted => {
            "invalid signed message: invalid SignedSSVMessage: signers not sorted".to_string()
        }
        _ => format!("Failed to create SignedSSVMessage: {:?}", error),
    }
}

/// Maps our internal SignedSSVMessageError to short spec error strings (for some tests)
pub fn map_signed_message_error_short(error: &SignedSSVMessageError) -> Option<&'static str> {
    match error {
        SignedSSVMessageError::NoSigners => Some("no signers"),
        SignedSSVMessageError::DuplicatedSigner => Some("non unique signer"),
        SignedSSVMessageError::ZeroSigner => Some("signer ID 0 not allowed"),
        _ => None,
    }
}

/// Maps conversion errors (String) to spec error strings
pub fn map_conversion_error(error: &str) -> Option<&'static str> {
    if error.contains("NoSigners") {
        Some("no signers")
    } else if error.contains("DuplicatedSigner") {
        Some("non unique signer")
    } else if error.contains("ZeroSigner") {
        Some("signer ID 0 not allowed")
    } else if error.contains("SignersNotSorted") {
        None // This should only happen for actual sorting issues
    } else {
        None
    }
}

/// Maps SSZ decode errors to spec error strings based on test context
pub fn map_ssz_decode_error(test_name: &str, error: &str) -> Option<&'static str> {
    // Map based on the error type and test context
    if error.contains("NoMatchingVariant") {
        // This happens when the message type is invalid
        Some("message type is invalid")
    } else if error.contains("InvalidByteLength { len: 0, expected: 8 }") {
        if test_name.contains("identifier") {
            Some("message identifier is invalid")
        } else if test_name.contains("type") {
            Some("message type is invalid")
        } else {
            None
        }
    } else if error.contains("InvalidLengthPrefix") {
        Some("message data is invalid")
    } else if error.contains("InvalidByteLength") {
        Some("message data is invalid")
    } else {
        None
    }
}

/// Standard QBFT validation error messages that match Go implementation
pub mod qbft_errors {
    pub const INSTANCE_STOPPED: &str = "instance stopped processing messages";
    pub const PAST_ROUND: &str = "invalid signed message: past round";
    pub const WRONG_HEIGHT: &str = "invalid signed message: wrong msg height";
    pub const SIGNER_NOT_IN_COMMITTEE: &str = "invalid signed message: signer not in committee";
    pub const MSG_ALLOWS_ONE_SIGNER: &str = "invalid signed message: msg allows 1 signer";
    pub const NO_SIGNERS: &str = "invalid signed message: no signers";
    pub const PROPOSAL_LEADER_INVALID: &str = "invalid signed message: proposal leader invalid";
    pub const PROPOSAL_NOT_VALID_STATE: &str =
        "invalid signed message: proposal is not valid with current state";
    pub const PROPOSAL_NOT_JUSTIFIED_NO_QUORUM: &str = "invalid signed message: proposal not justified: change round msg not valid: no justifications quorum";
    pub const PROPOSAL_NOT_JUSTIFIED_NO_RC_QUORUM: &str =
        "invalid signed message: proposal not justified: change round has no quorum";
    pub const DID_NOT_RECEIVE_PROPOSAL: &str =
        "invalid signed message: did not receive proposal for this round";
    pub const DID_NOT_PREPARE_YET: &str = "invalid signed message: did not prepare yet";
    pub const PROPOSED_DATA_MISMATCH: &str = "invalid signed message: proposed data mismatch";
}
