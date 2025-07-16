pub mod committee;
pub mod controller;
pub mod duty_execution;
pub mod message_processing;
pub mod new_duty;
pub mod partial_signatures;
pub mod runner_construction;
pub mod sync_committee_aggregator;
pub mod validation;

pub use committee::SsvCommitteeTest;
pub use controller::SsvControllerTest;
pub use duty_execution::SsvDutyExecutionTest;
pub use message_processing::SsvMessageProcessingTest;
pub use new_duty::SsvNewDutyTest;
pub use partial_signatures::SsvPartialSignatureTest;
pub use runner_construction::SsvRunnerConstructionTest;
pub use sync_committee_aggregator::SsvSyncCommitteeAggregatorTest;
pub use validation::SsvValidationTest;

use crate::SpecTest;
use std::fmt;

// SSV-specific test type enumeration
#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub enum SsvSpecTestType {
    Controller,
    MessageProcessing,
    MultiMessageProcessing,
    Committee,
    PartialSignatures,
    Validation,
    DutyExecution,
    RunnerConstruction,
    SyncCommitteeAggregator,
    NewDuty,
}

impl SsvSpecTestType {
    pub fn directory_name(&self) -> &'static str {
        match self {
            Self::Controller => "controller",
            Self::MessageProcessing => "runner",
            Self::MultiMessageProcessing => "runner",
            Self::Committee => "committee",
            Self::PartialSignatures => "partialsigcontainer",
            Self::Validation => "valcheck",
            Self::DutyExecution => "dutyexe",
            Self::RunnerConstruction => "runnerconstruction",
            Self::SyncCommitteeAggregator => "synccommitteeaggregator",
            Self::NewDuty => "newduty",
        }
    }
}

impl fmt::Display for SsvSpecTestType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Controller => write!(f, "controller"),
            Self::MessageProcessing => write!(f, "runner"),
            Self::MultiMessageProcessing => write!(f, "runner"),
            Self::Committee => write!(f, "committee"),
            Self::PartialSignatures => write!(f, "partialsigcontainer"),
            Self::Validation => write!(f, "valcheck"),
            Self::DutyExecution => write!(f, "dutyexe"),
            Self::RunnerConstruction => write!(f, "runnerconstruction"),
            Self::SyncCommitteeAggregator => write!(f, "synccommitteeaggregator"),
            Self::NewDuty => write!(f, "newduty"),
        }
    }
}

pub fn create_ssv_test(
    test_type: SsvSpecTestType,
    json_data: &str,
) -> Result<Box<dyn SpecTest>, serde_json::Error> {
    match test_type {
        SsvSpecTestType::Controller => {
            let test: SsvControllerTest = serde_json::from_str(json_data)?;
            Ok(Box::new(test))
        }
        SsvSpecTestType::MessageProcessing => {
            let test: SsvMessageProcessingTest = serde_json::from_str(json_data)?;
            Ok(Box::new(test))
        }
        SsvSpecTestType::MultiMessageProcessing => {
            let test: SsvMessageProcessingTest = serde_json::from_str(json_data)?;
            Ok(Box::new(test))
        }
        SsvSpecTestType::Committee => {
            let test: SsvCommitteeTest = serde_json::from_str(json_data)?;
            Ok(Box::new(test))
        }
        SsvSpecTestType::PartialSignatures => {
            let test: SsvPartialSignatureTest = serde_json::from_str(json_data)?;
            Ok(Box::new(test))
        }
        SsvSpecTestType::Validation => {
            let test: SsvValidationTest = serde_json::from_str(json_data)?;
            Ok(Box::new(test))
        }
        SsvSpecTestType::DutyExecution => {
            let test: SsvDutyExecutionTest = serde_json::from_str(json_data)?;
            Ok(Box::new(test))
        }
        SsvSpecTestType::RunnerConstruction => {
            let test: SsvRunnerConstructionTest = serde_json::from_str(json_data)?;
            Ok(Box::new(test))
        }
        SsvSpecTestType::SyncCommitteeAggregator => {
            let test: SsvSyncCommitteeAggregatorTest = serde_json::from_str(json_data)?;
            Ok(Box::new(test))
        }
        SsvSpecTestType::NewDuty => {
            let test: SsvNewDutyTest = serde_json::from_str(json_data)?;
            Ok(Box::new(test))
        }
    }
}
