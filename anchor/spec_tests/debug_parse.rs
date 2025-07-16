use std::fs;
use anchor_spec_tests::ssv::message_processing::SsvMessageProcessingTest;

fn main() {
    let test_file = "ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_no_signers.json";
    let json_data = fs::read_to_string(test_file).unwrap();
    
    match serde_json::from_str::<SsvMessageProcessingTest>(&json_data) {
        Ok(test) => {
            println!("Successfully parsed test: {}", test.name);
        }
        Err(e) => {
            println!("Error parsing test: {}", e);
        }
    }
}