/// Errors that can occur when building a QBFT config
#[derive(Debug, Clone)]
pub enum ConfigBuilderError {
    NoParticipants,
    OperatorNotParticipant,
    InvalidQuorumSize,
    ZeroMaxRounds,
    ExceedingStartingRound,
}

/// Errors that can occur during QBFT consensus
#[derive(Debug, Clone, PartialEq)]
pub enum QbftError {
    // Message validation errors
    InvalidSignature,
    SignerNotInCommittee,
    DuplicateSigners,
    WrongHeight,
    WrongRound,
    PastRound,
    InvalidMessageType,
    InvalidFullData,
    DataValidationFailed,
    NoData,

    // Proposal errors
    ProposalNotFromLeader,
    ProposalAlreadyReceived,
    ProposalMissingData,
    DuplicateProposal,

    // Justification errors
    RoundChangeJustificationNoQuorum,
    RoundChangeJustificationWrongRound,
    RoundChangeJustificationInvalidMessage,
    RoundChangeJustificationDecodeFailed,
    RoundChangeJustificationNotRoundChange,
    RoundChangeJustificationValidationFailed,
    RoundChangeJustificationInvalidSignature,
    RoundChangeJustificationDuplicateMsg,
    RoundChangeJustificationInvalidPrepares,
    RoundChangeJustificationInvalidPrepareRound,
    RoundChangeJustificationInvalidPrepareRoot,
    StandaloneRoundChangeNoQuorum,
    RoundChangeJustificationMultiSigner,
    PrepareJustificationWrongRound,
    PrepareJustificationNotEnough,
    PrepareJustificationValueMismatch,
    PrepareJustificationDecodeFailed,
    PrepareJustificationNotPrepare,
    PrepareJustificationValidationFailed,
    PrepareJustificationRootMismatch,
    PrepareJustificationInvalidValue,
    ProposalInvalidValue,

    // State errors
    InstanceAlreadyDecided,
    InvalidState,
    NoProposalAccepted,
    NotPreparedYet,
    ProposedDataMismatch,

    // Message format errors
    NoSigners,
    MultipleSignersNotAllowed,
    WrongMessageType,
    NotEnoughSignatures,
    InvalidJustification,

    // Other errors
    ForceStopped,
    RoundCutoff,
    Unknown(String),
}
