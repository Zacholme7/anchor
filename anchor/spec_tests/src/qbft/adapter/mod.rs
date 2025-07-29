pub mod error_mapping;
pub mod qbft_manager_test;
pub mod shared;
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

// Simple controller test adapter
pub use simple_controller_test::SimpleControllerTestAdapter;

// Error mapping utilities
pub use error_mapping::map_signed_ssv_error_to_go_format;

// Shared utilities for adapter consolidation
pub use shared::{
    SerializableCommitteeMember, SerializableOperator, base64_serde,
    validate_committee_configuration,
};
