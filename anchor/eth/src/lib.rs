pub use sync::{Config, SsvEventSyncer};
pub use util::parse_shares;
mod error;
mod event_parser;
mod event_processor;
mod gen;
mod network_actions;
mod sync;
mod util;
