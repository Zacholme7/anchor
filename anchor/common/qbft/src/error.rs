/// Error associated with Config building.
#[derive(Debug, Clone)]
pub enum ConfigBuilderError {
    /// No participants were specified
    NoParticipants,
    /// At least one round must be done
    ZeroMaxRounds,
    /// Starting round exceeds maximum rounds
    ExceedingStartingRound,
    /// Quorum must be in \[2f+1, participants-f\]
    InvalidQuorumSize,
    /// Operator ID must be specified
    MissingOperatorId,
    /// Operator ID must be contained in participants
    OperatorNotParticipant,
    /// Instance Height must be specified
    MissingInstanceHeight,
}

impl std::error::Error for ConfigBuilderError {}

impl std::fmt::Display for ConfigBuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::NoParticipants => {
                write!(f, "No participants were specified")
            }
            Self::ZeroMaxRounds => {
                write!(f, "At least one round must be done")
            }
            Self::ExceedingStartingRound => {
                write!(f, "Starting round exceeds maximum rounds")
            }
            Self::InvalidQuorumSize => {
                write!(f, "Quorum must be in [2f+1, participants-f]")
            }
            Self::MissingOperatorId => {
                write!(f, "Operator ID must be specified")
            }
            Self::OperatorNotParticipant => {
                write!(f, "Operator ID must be contained in participants")
            }
            Self::MissingInstanceHeight => {
                write!(f, "Instance height must be specified")
            }
        }
    }
}

/// Configuration for test scenarios
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub committee_size: usize,
    pub quorum_threshold: usize,
    pub max_rounds: u64,
    pub instance_height: u64,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            committee_size: 4,
            quorum_threshold: 3,
            max_rounds: 100,
            instance_height: 0,
        }
    }
}

/// Errors specific to test operations
#[derive(Debug, Clone)]
pub enum TestError {
    /// Invalid state for the requested operation
    InvalidState(String),
    /// Message creation failed
    MessageCreationFailed(String),
    /// Justification error
    JustificationError(String),
    /// Signing error
    SigningError(String),
    /// Scenario setup error
    ScenarioSetupError(String),
}

impl std::error::Error for TestError {}

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            Self::MessageCreationFailed(msg) => write!(f, "Message creation failed: {}", msg),
            Self::JustificationError(msg) => write!(f, "Justification error: {}", msg),
            Self::SigningError(msg) => write!(f, "Signing error: {}", msg),
            Self::ScenarioSetupError(msg) => write!(f, "Scenario setup error: {}", msg),
        }
    }
}
