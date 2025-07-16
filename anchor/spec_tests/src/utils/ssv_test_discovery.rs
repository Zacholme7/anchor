use crate::ssv::SsvSpecTestType;

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
    } else if filename.starts_with("runnerconstruction.") {
        Some(SsvSpecTestType::RunnerConstruction)
    } else if filename.starts_with("synccommitteeaggregator.") {
        Some(SsvSpecTestType::SyncCommitteeAggregator)
    } else if filename.starts_with("newduty.") {
        Some(SsvSpecTestType::NewDuty)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_ssv_test_type() {
        // Committee tests
        assert_eq!(
            determine_ssv_test_type("committee.CommitteeSpecTest_empty_committee_duty.json"),
            Some(SsvSpecTestType::Committee)
        );

        // Message processing tests
        assert_eq!(
            determine_ssv_test_type(
                "tests.MsgProcessingSpecTest_decide_on_slashable_attestation.json"
            ),
            Some(SsvSpecTestType::MessageProcessing)
        );

        // Multi message processing tests
        assert_eq!(
            determine_ssv_test_type(
                "tests.MultiMsgProcessingSpecTest_multi_decide_on_slashable_attestation.json"
            ),
            Some(SsvSpecTestType::MultiMessageProcessing)
        );

        // Validation tests
        assert_eq!(
            determine_ssv_test_type("valcheck.SpecTest_attestation_value_check_valid.json"),
            Some(SsvSpecTestType::Validation)
        );

        // Partial signatures tests
        assert_eq!(
            determine_ssv_test_type(
                "partialsigcontainer.PartialSigContainerTest_aggregation_sync_committee_agg.json"
            ),
            Some(SsvSpecTestType::PartialSignatures)
        );

        // New duty tests
        assert_eq!(
            determine_ssv_test_type(
                "newduty.MultiStartNewRunnerDutySpecTest_sync_committee_agg.json"
            ),
            Some(SsvSpecTestType::NewDuty)
        );

        // Runner construction tests
        assert_eq!(
            determine_ssv_test_type("runnerconstruction.RunnerConstructionSpecTest_one_share.json"),
            Some(SsvSpecTestType::RunnerConstruction)
        );

        // Sync committee aggregator tests
        assert_eq!(
            determine_ssv_test_type(
                "synccommitteeaggregator.SyncCommitteeAggregatorProofSpecTest_sync_committee_agg.json"
            ),
            Some(SsvSpecTestType::SyncCommitteeAggregator)
        );

        // Unknown file
        assert_eq!(determine_ssv_test_type("unknown.file.json"), None);
    }
}
