use serde_json::{Map, Value};
use sha2::{Sha256, Digest};
use std::collections::HashSet;
use std::fs;

#[derive(Debug, Clone)]
pub struct DebugReport {
    pub hash_match: bool,
    pub our_hash: String,
    pub expected_hash: String,
    pub field_order_diff: Vec<String>,
    pub missing_fields: Vec<String>,
    pub extra_fields: Vec<String>,
    pub structure_analysis: String,
    pub json_length: usize,
}

#[derive(Debug, Clone)]
pub struct FieldDifference {
    pub path: String,
    pub our_value: String,
    pub expected_value: String,
}

#[derive(Debug, Clone)]
pub struct HashSteps {
    pub json_string: String,
    pub json_bytes: Vec<u8>,
    pub hash_bytes: Vec<u8>,
    pub hash_hex: String,
}

/// Parse Go state comparison file to get expected JSON structure
pub fn parse_go_state_file(test_name: &str) -> Result<Value, String> {
    // Map test names to their corresponding Go state files
    let file_path = match test_name {
        name if name.contains("late commit") && !name.contains("past") => {
            "/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/ssv-spec/qbft/spectest/generate/state_comparison/tests_ControllerSpecTest/qbft controller late commit.json"
        }
        name if name.contains("decide current instance") && !name.contains("future") && !name.contains("past") => {
            "/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/ssv-spec/qbft/spectest/generate/state_comparison/tests_ControllerSpecTest/qbft controller decide current instance.json"
        }
        name if name.contains("late round change") && !name.contains("past") => {
            "/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/ssv-spec/qbft/spectest/generate/state_comparison/tests_ControllerSpecTest/qbft controller late round change.json"
        }
        _ => {
            return Err(format!("No Go state file mapping found for test: {}", test_name));
        }
    };

    match fs::read_to_string(file_path) {
        Ok(content) => {
            match serde_json::from_str::<Value>(&content) {
                Ok(json) => Ok(json),
                Err(e) => Err(format!("Failed to parse Go state file JSON: {}", e))
            }
        }
        Err(e) => Err(format!("Failed to read Go state file '{}': {}", file_path, e))
    }
}

/// Compare our JSON structure with expected hash and provide detailed analysis
pub fn compare_json_structures(our_json: &str, expected_hash: &str, test_name: &str) -> DebugReport {
    // Calculate our hash
    let hash = Sha256::digest(our_json.as_bytes());
    let our_hash = hex::encode(hash);
    
    // Parse our JSON
    let our_json_value: Value = serde_json::from_str(our_json).unwrap_or(Value::Null);
    
    // Try to get expected structure from Go state file
    let go_structure = parse_go_state_file(test_name);
    
    let mut field_order_diff = Vec::new();
    let mut missing_fields = Vec::new();
    let mut extra_fields = Vec::new();
    let mut structure_analysis = String::new();
    
    if let Ok(expected_json) = go_structure {
        // Analyze differences
        analyze_structure_differences(&our_json_value, &expected_json, "", &mut field_order_diff, &mut missing_fields, &mut extra_fields);
        
        // Field order analysis for top-level controller structure
        if let (Some(our_obj), Some(exp_obj)) = (our_json_value.as_object(), expected_json.as_array()) {
            // Go state files are arrays, first element is the controller
            if let Some(exp_controller) = exp_obj.first().and_then(|v| v.as_object()) {
                let our_keys: Vec<&String> = our_obj.keys().collect();
                let exp_keys: Vec<&String> = exp_controller.keys().collect();
                
                structure_analysis = format!(
                    "Our field order: {:?}\nExpected field order: {:?}\nField count - Our: {}, Expected: {}",
                    our_keys, exp_keys, our_keys.len(), exp_keys.len()
                );
                
                // Check if field order matches
                if our_keys != exp_keys {
                    field_order_diff.push("Top-level field order mismatch".to_string());
                }
            }
        }
    } else {
        structure_analysis = format!("Could not load Go state file for test '{}': {:?}", test_name, go_structure.err());
    }
    
    DebugReport {
        hash_match: our_hash == expected_hash,
        our_hash,
        expected_hash: expected_hash.to_string(),
        field_order_diff,
        missing_fields,
        extra_fields,
        structure_analysis,
        json_length: our_json.len(),
    }
}

/// Recursively analyze structural differences between JSON objects
fn analyze_structure_differences(
    our: &Value,
    expected: &Value,
    path: &str,
    field_order_diff: &mut Vec<String>,
    missing_fields: &mut Vec<String>,
    extra_fields: &mut Vec<String>,
) {
    match (our, expected) {
        (Value::Object(our_obj), Value::Object(exp_obj)) => {
            // Check for missing fields
            for key in exp_obj.keys() {
                if !our_obj.contains_key(key) {
                    missing_fields.push(format!("{}.{}", path, key));
                }
            }
            
            // Check for extra fields  
            for key in our_obj.keys() {
                if !exp_obj.contains_key(key) {
                    extra_fields.push(format!("{}.{}", path, key));
                }
            }
            
            // Recursively check common fields
            for key in exp_obj.keys() {
                if let Some(our_val) = our_obj.get(key) {
                    if let Some(exp_val) = exp_obj.get(key) {
                        let new_path = if path.is_empty() { key.clone() } else { format!("{}.{}", path, key) };
                        analyze_structure_differences(our_val, exp_val, &new_path, field_order_diff, missing_fields, extra_fields);
                    }
                }
            }
        }
        (Value::Array(our_arr), Value::Array(exp_arr)) => {
            if our_arr.len() != exp_arr.len() {
                field_order_diff.push(format!("{}[]: Array length mismatch - our: {}, expected: {}", path, our_arr.len(), exp_arr.len()));
            }
            
            // Check array elements
            for (i, (our_item, exp_item)) in our_arr.iter().zip(exp_arr.iter()).enumerate() {
                let new_path = format!("{}[{}]", path, i);
                analyze_structure_differences(our_item, exp_item, &new_path, field_order_diff, missing_fields, extra_fields);
            }
        }
        _ => {
            // For primitive values, check if they match
            if our != expected {
                field_order_diff.push(format!("{}: value mismatch", path));
            }
        }
    }
}

/// Analyze field differences between two JSON objects
pub fn analyze_field_differences(json1: &str, json2: &str) -> Vec<FieldDifference> {
    let mut differences = Vec::new();
    
    let val1: Value = serde_json::from_str(json1).unwrap_or(Value::Null);
    let val2: Value = serde_json::from_str(json2).unwrap_or(Value::Null);
    
    collect_field_differences(&val1, &val2, "", &mut differences);
    
    differences
}

fn collect_field_differences(val1: &Value, val2: &Value, path: &str, differences: &mut Vec<FieldDifference>) {
    match (val1, val2) {
        (Value::Object(obj1), Value::Object(obj2)) => {
            let all_keys: HashSet<&String> = obj1.keys().chain(obj2.keys()).collect();
            
            for key in all_keys {
                let new_path = if path.is_empty() { key.clone() } else { format!("{}.{}", path, key) };
                
                match (obj1.get(key), obj2.get(key)) {
                    (Some(v1), Some(v2)) => {
                        collect_field_differences(v1, v2, &new_path, differences);
                    }
                    (Some(v1), None) => {
                        differences.push(FieldDifference {
                            path: new_path,
                            our_value: format!("{}", v1),
                            expected_value: "MISSING".to_string(),
                        });
                    }
                    (None, Some(v2)) => {
                        differences.push(FieldDifference {
                            path: new_path,
                            our_value: "MISSING".to_string(),
                            expected_value: format!("{}", v2),
                        });
                    }
                    (None, None) => {} // Both missing, skip
                }
            }
        }
        (v1, v2) if v1 != v2 => {
            differences.push(FieldDifference {
                path: path.to_string(),
                our_value: format!("{}", v1),
                expected_value: format!("{}", v2),
            });
        }
        _ => {} // Values match
    }
}

/// Generate hash with step-by-step debugging information
pub fn generate_hash_step_by_step(json: &str) -> HashSteps {
    let json_bytes = json.as_bytes().to_vec();
    let hash_bytes = Sha256::digest(&json_bytes).to_vec();
    let hash_hex = hex::encode(&hash_bytes);
    
    HashSteps {
        json_string: json.to_string(),
        json_bytes,
        hash_bytes,
        hash_hex,
    }
}

/// Get the simplest failing test for systematic debugging
pub fn get_simplest_failing_test() -> &'static str {
    // Return the test name that represents the simplest failure case
    // Based on research, this is the best starting point
    "decide current instance"
}

/// Print detailed analysis of hash mismatch for debugging
pub fn print_debug_analysis(report: &DebugReport, test_name: &str) {
    println!("=== HASH DEBUG ANALYSIS: {} ===", test_name);
    println!("Hash Match: {}", report.hash_match);
    println!("Our Hash:      {}", report.our_hash);
    println!("Expected Hash: {}", report.expected_hash);
    println!("JSON Length: {}", report.json_length);
    
    if !report.field_order_diff.is_empty() {
        println!("Field Order Differences:");
        for diff in &report.field_order_diff {
            println!("  - {}", diff);
        }
    }
    
    if !report.missing_fields.is_empty() {
        println!("Missing Fields:");
        for field in &report.missing_fields {
            println!("  - {}", field);
        }
    }
    
    if !report.extra_fields.is_empty() {
        println!("Extra Fields:");
        for field in &report.extra_fields {
            println!("  - {}", field);
        }
    }
    
    if !report.structure_analysis.is_empty() {
        println!("Structure Analysis:");
        println!("{}", report.structure_analysis);
    }
    
    println!("=== END HASH DEBUG ANALYSIS ===");
}