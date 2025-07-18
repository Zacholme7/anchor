pub mod error_mapping;
pub mod keys;
pub mod types;
pub mod unified;

// Core unified adapter with integrated builder and scenario management
pub use unified::{
    MessageIdExt, QbftTestAdapter, extract_committee_from_spec_test, validate_committee,
    validate_root,
};

// Enhanced types with validation support
pub use types::{
    AdapterConfig, AdapterError, MessageCreationRequest, ScenarioResult, SpecTestCommitteeMember,
    TestContext, TestKeys, TestType, ValidationResult,
};

// Key management utilities
pub use keys::{
    create_minimal_test_keys, get_operator_ids, has_operator_key, load_operator_key,
    load_test_keys, load_test_keys_for_committee, validate_test_keys,
};

// Error mapping utilities
pub use error_mapping::{ErrorMapper, map_signed_ssv_error_to_go_format, simple_error_message};
