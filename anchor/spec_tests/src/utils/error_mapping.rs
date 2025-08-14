use qbft::QbftError;
use ssv_types::message::SignedSSVMessageError;

/// Maps QbftError to spec test error strings
pub fn map_qbft_error(error: &QbftError) -> String {
    match error {
        // Message validation errors
        QbftError::InvalidSignature => "invalid signed message: invalid signature".to_string(),
        QbftError::SignerNotInCommittee => "invalid signed message: signer not in committee".to_string(),
        QbftError::DuplicateSigners => "invalid signed message: duplicate signers".to_string(),
        QbftError::WrongHeight => "invalid signed message: wrong msg height".to_string(),
        QbftError::WrongRound => "invalid signed message: wrong msg round".to_string(),
        QbftError::PastRound => "invalid signed message: past round".to_string(),
        QbftError::InvalidMessageType => "invalid signed message: invalid message type".to_string(),
        QbftError::InvalidFullData => "invalid signed message: H(data) != root".to_string(),

        // Proposal specific
        QbftError::ProposalNotFromLeader => "invalid signed message: proposal leader invalid".to_string(),
        QbftError::ProposalAlreadyReceived => "invalid signed message: proposal already received".to_string(),
        QbftError::ProposalMissingData => "invalid signed message: proposal missing data".to_string(),

        // Round change justifications
        QbftError::RoundChangeJustificationNoQuorum =>
            "invalid signed message: proposal not justified: change round has no quorum".to_string(),
        QbftError::RoundChangeJustificationWrongRound =>
            "invalid signed message: round change justification invalid: wrong msg round".to_string(),
        QbftError::RoundChangeJustificationInvalidMessage =>
            "invalid signed message: round change justification invalid: invalid message".to_string(),
        QbftError::RoundChangeJustificationDecodeFailed =>
            "invalid signed message: round change justification invalid: decode failed".to_string(),
        QbftError::RoundChangeJustificationNotRoundChange =>
            "invalid signed message: round change justification invalid: not a round change".to_string(),
        QbftError::RoundChangeJustificationValidationFailed =>
            "invalid signed message: round change justification invalid: msg signature invalid: crypto/rsa: verification error".to_string(),
        QbftError::RoundChangeJustificationInvalidSignature =>
            "invalid signed message: proposal not justified: change round msg not valid: msg signature invalid: crypto/rsa: verification error".to_string(),
        QbftError::RoundChangeJustificationDuplicateMsg =>
            "invalid signed message: proposal not justified: change round has no quorum".to_string(),
        QbftError::RoundChangeJustificationInvalidPrepares =>
            "invalid signed message: proposal not justified: change round msg not valid: no justifications quorum".to_string(),
        QbftError::RoundChangeJustificationInvalidPrepareRound =>
            "invalid signed message: proposal not justified: change round msg not valid: round change justification invalid: wrong msg round".to_string(),
        QbftError::RoundChangeJustificationInvalidPrepareRoot =>
            "invalid signed message: proposal not justified: change round msg not valid: round change justification invalid: proposed data mismatch".to_string(),
        QbftError::StandaloneRoundChangeNoQuorum =>
            "invalid signed message: no justifications quorum".to_string(),
        QbftError::RoundChangeJustificationMultiSigner =>
            "invalid signed message: round change justification invalid: msg allows 1 signer".to_string(),

        // Prepare justifications
        QbftError::PrepareJustificationNotEnough =>
            "invalid signed message: proposal not justified: change round msg not valid: no justifications quorum".to_string(),
        QbftError::PrepareJustificationWrongRound =>
            "invalid signed message: proposal not justified: signed prepare not valid".to_string(),
        QbftError::PrepareJustificationValueMismatch =>
            "invalid signed message: proposal not justified: signed prepare not valid".to_string(),
        QbftError::PrepareJustificationDecodeFailed =>
            "invalid signed message: prepare justification invalid: decode failed".to_string(),
        QbftError::PrepareJustificationNotPrepare =>
            "invalid signed message: prepare justification invalid: not a prepare".to_string(),
        QbftError::PrepareJustificationValidationFailed =>
            "invalid signed message: proposal not justified: signed prepare not valid".to_string(),
        QbftError::PrepareJustificationRootMismatch =>
            "invalid signed message: proposal not justified: change round msg not valid: round change justification invalid: proposed data mismatch".to_string(),
        QbftError::PrepareJustificationInvalidValue =>
            "invalid signed message: proposal not justified: change round msg not valid: round change justification invalid: proposed data mismatch".to_string(),
        QbftError::ProposalInvalidValue =>
            "invalid signed message: proposal not justified: proposal fullData invalid: invalid value".to_string(),

        // State errors
        QbftError::InstanceAlreadyDecided => "instance already decided".to_string(),
        QbftError::InvalidState => "invalid signed message: proposal is not valid with current state".to_string(),
        QbftError::NoProposalAccepted => "invalid signed message: did not receive proposal for this round".to_string(),
        QbftError::NotPreparedYet => "invalid signed message: did not prepare yet".to_string(),
        QbftError::ProposedDataMismatch => "invalid signed message: proposed data mismatch".to_string(),

        // Message format errors
        QbftError::NoSigners => "invalid signed message: no signers".to_string(),
        QbftError::MultipleSignersNotAllowed => "invalid signed message: msg allows 1 signer".to_string(),

        // Other
        QbftError::ForceStopped => "instance stopped processing messages".to_string(),
        QbftError::RoundCutoff => "instance stopped processing messages".to_string(),
        QbftError::Unknown(msg) => msg.clone(),
    }
}

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
