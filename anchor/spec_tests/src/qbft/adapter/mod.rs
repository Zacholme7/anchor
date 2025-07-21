pub mod debug_tools;
pub mod error_mapping;
pub mod keys;
pub mod qbft_manager_test;
pub mod simple_controller_test;
pub mod types;
pub mod unified;

// Core unified adapter with integrated builder and scenario management
pub use unified::{
    MessageIdExt, QbftTestAdapter, extract_committee_from_spec_test, validate_committee,
    validate_root,
};

// Enhanced types with validation support
pub use types::{
    AdapterConfig, AdapterError, AsyncDecisionResult, AsyncScenarioResult, MessageCreationRequest,
    ScenarioResult, SpecTestCommitteeMember, TestContext, TestKeys, TestType, ValidationResult,
};

// Async QBFT manager test adapter
pub use qbft_manager_test::QbftManagerTestAdapter;

// Key management utilities
pub use keys::{
    create_minimal_test_keys, get_operator_ids, has_operator_key, load_operator_key,
    load_test_keys, load_test_keys_for_committee, validate_test_keys,
};

// Error mapping utilities
pub use error_mapping::{ErrorMapper, map_signed_ssv_error_to_go_format, simple_error_message};

// Debug utilities for systematic hash debugging
pub use debug_tools::{
    compare_json_structures, generate_hash_step_by_step, get_simplest_failing_test, 
    parse_go_state_file, print_debug_analysis, DebugReport
};
