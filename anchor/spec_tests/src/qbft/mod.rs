mod adapter;
mod controller_test;
mod create_message;
mod qbft_message;
mod round_robin;
// Removed modules:
// mod validation_adapter;
// mod validation_facade;

// Export adapter module for clean interface
pub use adapter::error_mapping::map_signed_ssv_error_to_go_format;

// Export test types
pub use controller_test::ControllerTest;
pub use create_message::CreateMessageTest;
pub use qbft_message::QbftMessageTest;
pub use round_robin::RoundRobinTest;

#[derive(Eq, PartialEq, Hash, Debug)]
pub(crate) enum QbftSpecTestType {
    QbftMessage,
    CreateMessage,
    RoundRobin,
    Controller,
}

// Contains specific identifier for the test file
impl std::fmt::Display for QbftSpecTestType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            QbftSpecTestType::QbftMessage => write!(f, "MsgSpecTest"),
            QbftSpecTestType::CreateMessage => write!(f, "CreateMsgSpecTest"),
            QbftSpecTestType::RoundRobin => write!(f, "RoundRobinSpecTest"),
            QbftSpecTestType::Controller => write!(f, "ControllerSpecTest"),
        }
    }
}
