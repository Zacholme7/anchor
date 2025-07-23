pub mod bridge;
pub mod debug_tools;
pub mod error_mapping;
pub mod keys;
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

// Async QBFT manager test adapter (with bridge layer integration)
pub use qbft_manager_test::QbftManagerTestAdapter;

// Simple controller test adapter (with bridge layer integration)
pub use simple_controller_test::SimpleControllerTestAdapter;

// Key management utilities
pub use keys::{
    create_minimal_test_keys, get_operator_ids, has_operator_key, load_operator_key,
    load_test_keys, load_test_keys_for_committee, validate_test_keys,
};

// Error mapping utilities
pub use error_mapping::{ErrorMapper, map_signed_ssv_error_to_go_format, simple_error_message};

// Debug utilities for systematic hash debugging
#[cfg(debug_assertions)]
pub use debug_tools::{
    compare_json_structures, generate_hash_step_by_step, get_simplest_failing_test, 
    parse_go_state_file, print_debug_analysis, DebugReport
};
#[cfg(not(debug_assertions))]
pub use debug_tools::{
    generate_hash_step_by_step, get_simplest_failing_test, 
    DebugReport
};

// Bridge layer exports for core QBFT integration
// Usage: Enable bridge layer with `.with_bridge_layer()` on any adapter
// Bridge components delegate QBFT business logic to production APIs:
//   - QbftBridge: Core QBFT consensus operations
//   - MessageBridge: Format conversion between JSON and Rust types  
//   - ValidationBridge: Message validation using production validator
//   - StateBridge: Controller state management via QBFT manager
pub use bridge::{
    QbftBridge, MessageBridge, ValidationBridge, StateBridge,
    TestMessageSender, MockDutiesProvider
};

// Shared utilities for adapter consolidation (enhanced with bridge utilities)
pub use shared::{
    SerializableController, SerializableCommitteeMember, SerializableOperator,
    base64_serde, optional_base64_serde, calculate_sha256_hash, 
    validate_committee_configuration, validate_message_structure,
    // Bridge utilities
    build_committee_from_spec_test, build_qbft_config_from_spec,
    build_message_id_for_spec_test, map_core_error_to_go_format,
};
