# QBFT Create Message Tests Fix Implementation Plan

## Executive Summary

> **Problem**: QBFT create message tests are failing due to new JSON structure in the `add_hardcoded_test_data` branch. The tests now include hardcoded committee data, operator keys, and additional metadata fields that our current Rust structs cannot parse.
>
> **Solution**: Update the `CreateMessageTest` struct to handle the new JSON fields and modify the test logic to use the hardcoded committee data instead of generated keys, matching the Go implementation's approach.
>
> **Technical Approach**: Remove the `deny_unknown_fields` restriction, add the new fields to the struct, and update the test execution logic to use the committee data from JSON instead of `TestKeySet`.
>
> **Expected Outcomes**: All QBFT create message tests pass using the same hardcoded data as the Go implementation, ensuring consistent test results.

## Goals & Objectives

### Primary Goals
- **Fix JSON Parsing**: Update struct to handle new fields without failing
- **Use Hardcoded Data**: Replace generated keys with JSON-provided committee data
- **Maintain Test Integrity**: Ensure tests verify the same logic as before but with consistent data

### Secondary Objectives
- **Align with Go Implementation**: Match the Go test pattern of using hardcoded committee data
- **Improve Test Reliability**: Use consistent data across runs instead of generated keys
- **Support Future JSON Changes**: Remove strict field validation to handle future additions

## Solution Overview

### Approach
Update the `CreateMessageTest` struct to accept the new JSON fields and modify the test logic to use the hardcoded committee data provided in the JSON files instead of locally generated keys.

### Key Components
1. **Struct Updates**: Add new fields and remove `deny_unknown_fields`
2. **Committee Data Parsing**: Add structures to handle the hardcoded committee data
3. **Test Logic Updates**: Use JSON-provided data instead of `TestKeySet`
4. **Key Management**: Parse base64 RSA keys from JSON

### Data Flow
```
JSON Test File → Parse Committee Data → Create QBFT Instance → Generate Message → Verify Root
                 (hardcoded keys)      (use JSON keys)     (same logic)    (matches expected)
```

### Expected Outcomes
- All QBFT create message tests pass
- Tests use consistent hardcoded data matching Go implementation
- Root verification succeeds because we use the same keys as expected roots

## Implementation Tasks

### CRITICAL IMPLEMENTATION RULES
1. **NO PLACEHOLDER CODE**: Every implementation must be production-ready
2. **COMPLETE IMPLEMENTATIONS**: Each task must fully implement its feature
3. **MAINTAIN BACKWARD COMPATIBILITY**: Don't break existing functionality
4. **USE EXACT JSON STRUCTURE**: Match the Go implementation's data usage

### Visual Dependency Tree
```
anchor/spec_tests/src/qbft/
├── create_message.rs (Task #0: Update CreateMessageTest struct and logic)
│   ├── CommitteeMember struct (new)
│   ├── Operator struct (new)
│   └── Updated test execution logic
└── qbft_deserializers.rs (Task #1: Add committee data deserializers)
    ├── base64_rsa_key_deserializer
    └── committee_member_deserializer
```

### Execution Plan

#### Group A: Structure Updates (Execute Task #0)
- [x] **Task #0**: Update CreateMessageTest struct and test logic
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `create_message.rs`
  - **Remove**: `#[serde(deny_unknown_fields)]` from `CreateMessageTest`
  - **Add Fields**:
    ```rust
    // New fields to handle hardcoded test data
    #[serde(rename = "Type")]
    pub test_type: Option<String>,
    
    #[serde(rename = "Documentation")]
    pub documentation: Option<String>,
    
    #[serde(rename = "CommitteeMember")]
    pub committee_member: Option<CommitteeMember>,
    
    #[serde(rename = "Identifier")]
    pub identifier: Option<String>,
    
    #[serde(rename = "OperatorID")]
    pub operator_id: Option<u64>,
    ```
  - **Add Structures**:
    ```rust
    #[derive(Deserialize)]
    pub struct CommitteeMember {
        #[serde(rename = "OperatorID")]
        pub operator_id: u64,
        
        #[serde(rename = "CommitteeID")]
        pub committee_id: Vec<u8>,
        
        #[serde(rename = "SSVOperatorPubKey")]
        pub ssv_operator_pub_key: String,
        
        #[serde(rename = "FaultyNodes")]
        pub faulty_nodes: u64,
        
        #[serde(rename = "Committee")]
        pub committee: Vec<Operator>,
        
        #[serde(rename = "DomainType")]
        pub domain_type: [u8; 4],
    }
    
    #[derive(Deserialize)]
    pub struct Operator {
        #[serde(rename = "OperatorID")]
        pub operator_id: u64,
        
        #[serde(rename = "SSVOperatorPubKey")]
        pub ssv_operator_pub_key: String,
    }
    ```
  - **Update `create_qbft_instance()` method**:
    ```rust
    fn create_qbft_instance(&self) -> Result<UnifiedTestAdapter, String> {
        // Use committee data from JSON if available, otherwise fallback to test keys
        let (committee, identifier) = if let Some(committee_member) = &self.committee_member {
            // Parse committee from JSON
            let committee: IndexSet<OperatorId> = committee_member.committee
                .iter()
                .map(|op| OperatorId::from(op.operator_id))
                .collect();
            
            // Use identifier from JSON if available
            let identifier = if let Some(id_str) = &self.identifier {
                // Parse base64 identifier
                MessageId::from_base64_or_default(id_str)
            } else {
                MessageId::for_spectest()
            };
            
            (committee, identifier)
        } else {
            // Fallback to existing behavior
            let four_share_set = TestKeySet::four_share_set();
            let committee: IndexSet<OperatorId> = four_share_set.operator_keys.keys().cloned().collect();
            let identifier = MessageId::for_spectest();
            (committee, identifier)
        };
        
        // Rest of existing logic...
    }
    ```
  - **Update signing logic**: Use RSA keys from JSON if available
  - **Context**: This enables the tests to use hardcoded data matching Go implementation
  - **Integration**: Works with existing test framework but uses JSON-provided data

#### Group B: Helper Functions (Execute in parallel with Group A)
- [x] **Task #1**: Add committee data parsing utilities
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `qbft_deserializers.rs` (or create new utilities file)
  - **Add Functions**:
    ```rust
    /// Parse base64-encoded RSA public key from JSON
    pub fn parse_base64_rsa_key(key_str: &str) -> Result<PKey<Private>, String> {
        // Decode base64 PEM format
        // Parse RSA key
        // Return PKey<Private>
    }
    
    /// Create MessageId from base64 string or use default
    pub fn message_id_from_base64_or_default(id_str: &str) -> MessageId {
        // Parse base64 identifier or return default
    }
    
    /// Create committee info from JSON committee member
    pub fn create_committee_info(committee_member: &CommitteeMember) -> CommitteeInfo {
        // Build CommitteeInfo from JSON data
    }
    ```
  - **Context**: Helper functions for parsing JSON data into Rust types
  - **Integration**: Used by `create_qbft_instance()` for data transformation

#### Group C: Testing and Validation (Execute after Groups A and B)
- [x] **Task #2**: Test and validate the implementation
  - **Run Tests**: Execute `cargo test -p spec_tests test_qbft_create` to verify fixes
  - **Validate Parsing**: Ensure all new fields are correctly parsed from JSON
  - **Verify Root Generation**: Confirm that using hardcoded keys produces expected roots
  - **Edge Case Testing**: Test both with and without committee data in JSON
  - **Regression Testing**: Ensure existing functionality still works

---

## Implementation Workflow

This plan file serves as the authoritative checklist for implementation. When implementing:

### Required Process
1. **Load Plan**: Read this entire plan file before starting
2. **Sync Tasks**: Create TodoWrite tasks matching the checkboxes below
3. **Execute & Update**: For each task:
   - Mark TodoWrite as `in_progress` when starting
   - Update checkbox `[ ]` to `[x]` when completing
   - Mark TodoWrite as `completed` when done
4. **Maintain Sync**: Keep this file and TodoWrite synchronized throughout

### Critical Rules
- This plan file is the source of truth for progress
- Update checkboxes in real-time as work progresses
- Never lose synchronization between plan file and TodoWrite
- Mark tasks complete only when fully implemented (no placeholders)
- Tasks should be run in parallel, unless there are dependencies, using subtasks, to avoid context bloat

### Progress Tracking
The checkboxes above represent the authoritative status of each task. Keep them updated as you work.