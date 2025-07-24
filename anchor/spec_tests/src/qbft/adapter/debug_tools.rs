use sha2::{Sha256, Digest};

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

/// Compare our JSON structure with expected hash and provide basic analysis
#[cfg(debug_assertions)]
pub fn compare_json_structures(our_json: &str, expected_hash: &str, _test_name: &str) -> DebugReport {
    // Calculate our hash
    let hash = Sha256::digest(our_json.as_bytes());
    let our_hash = hex::encode(hash);
    
    DebugReport {
        hash_match: our_hash == expected_hash,
        our_hash,
        expected_hash: expected_hash.to_string(),
        field_order_diff: Vec::new(),
        missing_fields: Vec::new(),
        extra_fields: Vec::new(),
        structure_analysis: "Simplified debug analysis".to_string(),
        json_length: our_json.len(),
    }
}

/// Generate hash with step-by-step debugging information
pub fn generate_hash_step_by_step(json: &str) -> (String, Vec<u8>, Vec<u8>, String) {
    let json_bytes = json.as_bytes().to_vec();
    let hash_bytes = Sha256::digest(&json_bytes).to_vec();
    let hash_hex = hex::encode(&hash_bytes);
    
    (json.to_string(), json_bytes, hash_bytes, hash_hex)
}

/// Get the simplest failing test for systematic debugging
pub fn get_simplest_failing_test() -> &'static str {
    "decide current instance"
}

/// Print simplified debug analysis (debug builds only)
#[cfg(debug_assertions)]
pub fn print_debug_analysis(report: &DebugReport, test_name: &str) {
    println!("=== HASH DEBUG: {} ===", test_name);
    println!("Hash Match: {}", report.hash_match);
    println!("Our Hash:      {}", report.our_hash);
    println!("Expected Hash: {}", report.expected_hash);
    println!("JSON Length: {}", report.json_length);
    println!("=== END DEBUG ===");
}

/// Print simplified debug analysis (release builds - no-op)
#[cfg(not(debug_assertions))]
pub fn print_debug_analysis(_report: &DebugReport, _test_name: &str) {
    // No debug output in release builds
}