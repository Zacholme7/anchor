use crate::ssv::message_processing::SsvMessageProcessingTest;
use std::fs;

#[test]
fn debug_multi_message_processing_parse() {
    let test_files = [
        "ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_no_signers.json",
        "ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_full_happy_flow.json",
        "ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_nil_SSVMessage.json",
        "ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_empty_signature.json",
        "ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_no_signatures.json",
    ];

    for test_file in test_files {
        println!("Processing file: {}", test_file);
        let json_data = fs::read_to_string(test_file).unwrap();

        match serde_json::from_str::<SsvMessageProcessingTest>(&json_data) {
            Ok(test) => {
                println!("  Successfully parsed test: {}", test.name);
            }
            Err(e) => {
                println!("  Error parsing test: {}", e);
                // Find the line number in the error message
                if let Some(line_start) = e.to_string().find("at line ") {
                    let line_part = &e.to_string()[line_start + 8..];
                    if let Some(space_pos) = line_part.find(' ') {
                        let line_num_str = &line_part[..space_pos];
                        if let Ok(line_num) = line_num_str.parse::<usize>() {
                            let lines: Vec<&str> = json_data.lines().collect();
                            if line_num > 0 && line_num <= lines.len() {
                                let start = (line_num.saturating_sub(3)).max(0);
                                let end = (line_num + 2).min(lines.len());
                                println!("  Context around line {}:", line_num);
                                for i in start..end {
                                    let marker = if i + 1 == line_num { ">>>" } else { "   " };
                                    println!("  {} {}: {}", marker, i + 1, lines[i]);
                                }
                            }
                        }
                    }
                }
            }
        }
        println!();
    }
}

#[test]
fn count_all_multi_message_processing_tests() {
    let entries = fs::read_dir("ssv-spec/ssv/spectest/generate/tests/").unwrap();
    let mut total_tests = 0;
    let mut passed_tests = 0;
    let mut failed_tests = 0;

    for entry in entries {
        let entry = entry.unwrap();
        let file_name = entry.file_name().to_string_lossy().to_string();

        if file_name.starts_with("tests.MultiMsgProcessingSpecTest_")
            && file_name.ends_with(".json")
        {
            total_tests += 1;

            match fs::read_to_string(entry.path()) {
                Ok(json_data) => {
                    match serde_json::from_str::<SsvMessageProcessingTest>(&json_data) {
                        Ok(_) => {
                            passed_tests += 1;
                            println!("✓ {}", file_name);
                        }
                        Err(e) => {
                            failed_tests += 1;
                            println!("✗ {}: {}", file_name, e);
                        }
                    }
                }
                Err(e) => {
                    failed_tests += 1;
                    println!("✗ {} (read error): {}", file_name, e);
                }
            }
        }
    }

    println!("\n=== Multi Message Processing Test Results ===");
    println!("Total tests: {}", total_tests);
    println!("Passed: {}", passed_tests);
    println!("Failed: {}", failed_tests);
    println!(
        "Success rate: {}/{} ({}%)",
        passed_tests,
        total_tests,
        if total_tests > 0 {
            (passed_tests * 100) / total_tests
        } else {
            0
        }
    );
}
