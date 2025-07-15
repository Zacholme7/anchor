use super::error_mapping::ErrorMapper;
use super::types::{AdapterError, SpecTestCommitteeMember, TestContext, ValidationResult};
use message_validator::{ValidationFailure, minimal_validate_consensus_message};
use ssv_types::{IndexSet, OperatorId, message::SignedSSVMessage};
use tree_hash::TreeHash;
use types::Hash256;

/// Enhanced message validation using message_validator
pub fn validate_message_comprehensive(
    msg: &SignedSSVMessage,
    committee: &IndexSet<OperatorId>,
    _context: &TestContext,
) -> ValidationResult {
    let mut errors = Vec::new();

    // Basic message validation using message_validator
    if let Err(failure) = minimal_validate_consensus_message(msg) {
        errors.push(format!("{:?}", failure));
    }

    // Committee-specific validation
    if let Err(failure) = validate_message_committee(msg, committee) {
        errors.push(format!("{:?}", failure));
    }

    ValidationResult {
        is_valid: errors.is_empty(),
        errors,
        warnings: Vec::new(),
    }
}

/// Basic message validation using message_validator
pub fn validate_message_basic(msg: &SignedSSVMessage) -> Result<(), ValidationFailure> {
    // Use message_validator instead of custom logic
    minimal_validate_consensus_message(msg).map(|_| ())
}

/// Enhanced justifications validation using message_validator
pub fn validate_justifications(
    justifications: &[SignedSSVMessage],
    committee: &IndexSet<OperatorId>,
) -> ValidationResult {
    let mut all_errors = Vec::new();

    for justification in justifications.iter() {
        if let Err(failure) = minimal_validate_consensus_message(justification) {
            all_errors.push(format!("{:?}", failure));
        }

        // Also validate committee membership
        if let Err(failure) = validate_message_committee(justification, committee) {
            all_errors.push(format!("{:?}", failure));
        }
    }

    ValidationResult {
        is_valid: all_errors.is_empty(),
        errors: all_errors,
        warnings: Vec::new(),
    }
}

/// Validate message tree hash root against expected value
pub fn validate_root(message: &SignedSSVMessage, expected: Hash256) -> bool {
    message.tree_hash_root() == expected
}

/// Extract committee from spec test JSON data
pub fn extract_committee_from_spec_test(
    committee_member: &SpecTestCommitteeMember,
) -> Result<IndexSet<OperatorId>, AdapterError> {
    let mut committee = IndexSet::new();

    // Extract committee from the committee info
    for operator in committee_member.committee.iter() {
        committee.insert(OperatorId::from(operator.operator_id));
    }

    if committee.is_empty() {
        return Err(AdapterError::Config("Empty committee".to_string()));
    }

    Ok(committee)
}

/// Validate committee configuration
pub fn validate_committee(
    committee: &IndexSet<OperatorId>,
    quorum_threshold: usize,
) -> Result<(), AdapterError> {
    if committee.is_empty() {
        return Err(AdapterError::Config(
            "Committee cannot be empty".to_string(),
        ));
    }

    if quorum_threshold == 0 {
        return Err(AdapterError::Config(
            "Quorum threshold cannot be zero".to_string(),
        ));
    }

    if quorum_threshold > committee.len() {
        return Err(AdapterError::Config(format!(
            "Quorum threshold {} exceeds committee size {}",
            quorum_threshold,
            committee.len()
        )));
    }

    // Check that all operator IDs are non-zero
    for operator_id in committee {
        if operator_id.0 == 0 {
            return Err(AdapterError::Config(
                "Committee contains zero operator ID".to_string(),
            ));
        }
    }

    Ok(())
}

/// Validate message against committee membership
pub fn validate_message_committee(
    message: &SignedSSVMessage,
    committee: &IndexSet<OperatorId>,
) -> Result<(), ValidationFailure> {
    let operator_ids = message.operator_ids();

    for operator_id in operator_ids {
        if !committee.contains(operator_id) {
            return Err(ValidationFailure::SignerNotInCommittee);
        }
    }

    Ok(())
}

/// Enhanced validation with Go error formatting
pub fn validate_and_format_errors(
    msg: &SignedSSVMessage,
    committee: &IndexSet<OperatorId>,
    context: &TestContext,
) -> (ValidationResult, Vec<String>) {
    let validation_result = validate_message_comprehensive(msg, committee, context);
    let error_mapper = ErrorMapper::new(context.clone());
    let go_formatted_errors = error_mapper.map_validation_error_strings(&validation_result.errors);

    (validation_result, go_formatted_errors)
}

/// Advanced validation combining multiple checks
pub fn validate_message_full(
    message: &SignedSSVMessage,
    committee: &IndexSet<OperatorId>,
    expected_root: Option<Hash256>,
) -> Result<(), ValidationFailure> {
    // Basic validation
    validate_message_basic(message)?;

    // Committee membership validation
    validate_message_committee(message, committee)?;

    // Root validation if expected
    if let Some(expected) = expected_root {
        if !validate_root(message, expected) {
            return Err(ValidationFailure::InvalidHash);
        }
    }

    Ok(())
}
