pub mod committee;
pub mod message_processing;
pub mod new_duty;
pub mod partial_signatures;
pub mod sync_committee_aggregator;
pub mod validation;

pub use committee::SsvCommitteeTest;
pub use message_processing::{SsvMessageProcessingTest, SsvMultiMessageProcessingTest};
pub use new_duty::SsvNewDutyTest;
pub use partial_signatures::SsvPartialSignatureTest;
pub use sync_committee_aggregator::SsvSyncCommitteeAggregatorTest;
pub use validation::SsvValidationTest;

use std::fmt;

// SSV-specific test type enumeration
#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub enum SsvSpecTestType {
    MessageProcessing,
    MultiMessageProcessing,
    Committee,
    PartialSignatures,
    Validation,
    SyncCommitteeAggregator,
    NewDuty,
}

impl SsvSpecTestType {
    pub fn directory_name(&self) -> &'static str {
        match self {
            Self::MessageProcessing => "runner",
            Self::MultiMessageProcessing => "runner",
            Self::Committee => "committee",
            Self::PartialSignatures => "partialsigcontainer",
            Self::Validation => "valcheck",
            Self::SyncCommitteeAggregator => "synccommitteeaggregator",
            Self::NewDuty => "newduty",
        }
    }
}

impl fmt::Display for SsvSpecTestType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::MessageProcessing => write!(f, "runner"),
            Self::MultiMessageProcessing => write!(f, "runner"),
            Self::Committee => write!(f, "committee"),
            Self::PartialSignatures => write!(f, "partialsigcontainer"),
            Self::Validation => write!(f, "valcheck"),
            Self::SyncCommitteeAggregator => write!(f, "synccommitteeaggregator"),
            Self::NewDuty => write!(f, "newduty"),
        }
    }
}

/// Determine the SSV test type based on the filename pattern
pub fn determine_ssv_test_type(filename: &str) -> Option<SsvSpecTestType> {
    if filename.starts_with("committee.") {
        Some(SsvSpecTestType::Committee)
    } else if filename.starts_with("tests.MultiMsgProcessingSpecTest_") {
        Some(SsvSpecTestType::MultiMessageProcessing)
    } else if filename.starts_with("tests.MsgProcessingSpecTest_") {
        Some(SsvSpecTestType::MessageProcessing)
    } else if filename.starts_with("tests.") {
        // Default for other tests.* files
        Some(SsvSpecTestType::MessageProcessing)
    } else if filename.starts_with("partialsigcontainer.") {
        Some(SsvSpecTestType::PartialSignatures)
    } else if filename.starts_with("valcheck.") {
        Some(SsvSpecTestType::Validation)
    } else if filename.starts_with("synccommitteeaggregator.") {
        Some(SsvSpecTestType::SyncCommitteeAggregator)
    } else if filename.starts_with("newduty.") {
        Some(SsvSpecTestType::NewDuty)
    } else {
        None
    }
}
