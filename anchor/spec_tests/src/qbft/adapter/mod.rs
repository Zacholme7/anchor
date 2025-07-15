pub mod error_mapping;
pub mod types;
pub mod unified;
pub mod validation;

// Core unified adapter with integrated builder and scenario management
pub use unified::QbftTestAdapter;

// Enhanced types with validation support
pub use types::{
    AdapterError, MessageCreationRequest, ScenarioResult, SpecTestCommitteeMember, TestContext,
    TestType,
};
