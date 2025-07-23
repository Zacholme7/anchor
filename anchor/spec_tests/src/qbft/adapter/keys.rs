use super::types::{AdapterError, TestKeys};
use crate::utils::test_keys::TestKeySet;
use openssl::rsa::Rsa;
use ssv_types::OperatorId;
use std::collections::HashMap;

/// Load test keys from the standard test key set
///
/// This function creates a TestKeys structure from the comprehensive TestKeySet
/// used throughout the spec tests, providing a bridge between the full key infrastructure
/// and the simplified adapter key structure.
pub fn load_test_keys() -> Result<TestKeys, AdapterError> {
    let test_key_set = TestKeySet::four_share_set();

    // Convert from TestKeySet to TestKeys
    let operator_keys = test_key_set.operator_keys;
    let committee_size = test_key_set.share_count as usize;

    Ok(TestKeys {
        operator_keys,
        committee_size,
    })
}

/// Load a specific operator key from the test key set
///
/// This function retrieves a specific operator's RSA private key from the standard
/// test key set, useful for targeted testing scenarios.
pub fn load_operator_key(
    operator_id: OperatorId,
) -> Result<Rsa<openssl::pkey::Private>, AdapterError> {
    let test_keys = load_test_keys()?;

    test_keys.get_key(operator_id).cloned().ok_or_else(|| {
        AdapterError::KeyLoading(format!("Operator key not found for ID: {}", operator_id))
    })
}

/// Load test keys for a specific committee size
///
/// This function creates a TestKeys structure with the specified number of operators.
/// Currently supports only 4-operator committees, but can be extended for other sizes.
pub fn load_test_keys_for_committee(committee_size: usize) -> Result<TestKeys, AdapterError> {
    match committee_size {
        4 => load_test_keys(),
        _ => Err(AdapterError::KeyLoading(format!(
            "Unsupported committee size: {}. Only 4-operator committees are currently supported",
            committee_size
        ))),
    }
}

/// Create a minimal test key set for testing purposes
///
/// This function creates a simplified TestKeys structure with generated RSA keys.
/// It's useful for scenarios where you don't need the full spec test key infrastructure.
pub fn create_minimal_test_keys(committee_size: usize) -> Result<TestKeys, AdapterError> {
    let mut operator_keys = HashMap::new();

    for operator_id in 1..=committee_size {
        let rsa_key = Rsa::generate(1024).map_err(|e| {
            AdapterError::KeyLoading(format!(
                "Failed to generate RSA key for operator {}: {}",
                operator_id, e
            ))
        })?;

        operator_keys.insert(OperatorId::from(operator_id as u64), rsa_key);
    }

    Ok(TestKeys {
        operator_keys,
        committee_size,
    })
}

/// Validate that all required operator keys are present
///
/// This function checks that the TestKeys structure contains all necessary keys
/// for the specified committee size and that all keys are valid.
pub fn validate_test_keys(test_keys: &TestKeys) -> Result<(), AdapterError> {
    if test_keys.operator_keys.len() != test_keys.committee_size {
        return Err(AdapterError::KeyLoading(format!(
            "Key count mismatch: expected {} keys, found {}",
            test_keys.committee_size,
            test_keys.operator_keys.len()
        )));
    }

    // Check that we have keys for operators 1 through committee_size
    for operator_id in 1..=test_keys.committee_size {
        let id = OperatorId::from(operator_id as u64);
        if !test_keys.operator_keys.contains_key(&id) {
            return Err(AdapterError::KeyLoading(format!(
                "Missing key for operator {}",
                operator_id
            )));
        }
    }

    Ok(())
}

/// Get all operator IDs from the test keys
///
/// This function returns a vector of all operator IDs present in the TestKeys structure.
pub fn get_operator_ids(test_keys: &TestKeys) -> Vec<OperatorId> {
    test_keys.operator_keys.keys().cloned().collect()
}

/// Check if a specific operator key exists
///
/// This function checks whether a key exists for the specified operator ID.
pub fn has_operator_key(test_keys: &TestKeys, operator_id: OperatorId) -> bool {
    test_keys.operator_keys.contains_key(&operator_id)
}

