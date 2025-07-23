//! Bridge layer connecting QBFT spec test adapters to core production APIs
//! 
//! This module provides integration utilities that allow test adapters to delegate
//! business logic to the core QBFT implementation while maintaining spec test compatibility.

pub mod qbft_bridge;
pub mod message_bridge;
pub mod validation_bridge;
pub mod state_bridge;
pub mod message_processing_bridge;

// Re-export bridge components for easy access
pub use qbft_bridge::{QbftBridge, TestMessageSender};
pub use message_bridge::MessageBridge;
pub use validation_bridge::{ValidationBridge, MockDutiesProvider};
pub use state_bridge::StateBridge;
pub use message_processing_bridge::{MessageProcessingBridge, MessageProcessingResult, QbftInstanceState};