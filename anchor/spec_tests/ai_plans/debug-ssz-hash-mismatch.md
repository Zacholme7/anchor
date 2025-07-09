# SSZ Hash Mismatch Root Cause Analysis Plan

## Executive Summary
> **Problem Statement**: QBFT spec tests are failing due to systematic tree hash mismatches between Rust and Go implementations. While individual field values appear correct and RSA signing is deterministic, the final tree hash calculations differ significantly: Rust produces `44179cfd...` while Go produces `43c23219...`. Manual SHA256 of concatenated field hashes yields `c325f78a...`, suggesting fundamental differences in SSZ tree hash computation between implementations.
>
> **Proposed Solution**: Implement a comprehensive root cause analysis system that systematically compares SSZ tree hash computation at every level - from individual field hashing to final tree root calculation. This includes byte-level comparison tools, field-by-field tree hash validation, and step-by-step debugging of the SSZ merkleization process to identify the exact point of divergence.
>
> **Technical Approach**: Create parallel debugging infrastructure to extract and compare Go vs Rust SSZ tree hash computation at each step, implement comprehensive field-level tree hash comparison, and build tools to trace the exact merkleization process differences between `fastssz` and `tree_hash` implementations.
>
> **Expected Outcomes**: Identify the exact byte-level difference causing hash mismatches, understand the root cause of tree hash computation differences, and establish a clear path to fix the SSZ compatibility issues.

## Goals & Objectives
### Primary Goals
- **Identify exact root cause**: Pinpoint the specific step in SSZ tree hash computation where Rust and Go diverge
- **Establish systematic debugging**: Create comprehensive tools to compare tree hash computation at every level
- **Enable parallel investigation**: Structure debugging to simultaneously test multiple theories about the root cause

### Secondary Objectives
- **Build reusable debugging infrastructure**: Create tools for future SSZ compatibility issues
- **Document tree hash differences**: Create clear understanding of `fastssz` vs `tree_hash` implementation differences
- **Validate individual field correctness**: Confirm that field values themselves are correct before tree hashing

## Solution Overview
### Approach
The solution systematically debugs SSZ tree hash computation by comparing Rust and Go implementations at every level of the merkleization process. This includes extracting raw field data, comparing individual field tree hashes, analyzing the merkleization tree structure, and identifying the exact point where hash calculations diverge.

### Key Components
1. **Go SSZ Extraction Tools**: Extract raw SSZ data and intermediate tree hash values from Go implementation
2. **Rust SSZ Debugging Tools**: Detailed analysis of Rust tree hash computation with step-by-step logging
3. **Field-Level Comparison**: Byte-by-byte comparison of individual field tree hashes between implementations
4. **Merkleization Process Tracing**: Step-by-step analysis of tree hash computation differences
5. **Parallel Theory Testing**: Simultaneous investigation of multiple potential root causes

### Data Flow
```
Same Input Data → Go Implementation → Extract Tree Hash Steps → Compare Each Step → Identify Divergence Point
                ↓                                                     ↑
                Rust Implementation → Extract Tree Hash Steps → Compare Each Step → Root Cause Analysis
```

### Investigation Theories
1. **Field Tree Hash Differences**: Individual field `tree_hash()` implementations differ between Rust and Go
2. **Merkleization Algorithm Differences**: `fastssz` vs `tree_hash` use different merkleization approaches
3. **Field Ordering/Structure**: Subtle differences in how fields are arranged before tree hashing
4. **Padding/Alignment**: Different padding or alignment applied during tree hash computation

### Expected Outcomes
- Exact identification of where tree hash computation diverges
- Clear understanding of `fastssz` vs `tree_hash` implementation differences
- Actionable plan to fix SSZ compatibility issues
- Comprehensive debugging tools for future compatibility issues

## Implementation Tasks

### CRITICAL IMPLEMENTATION RULES
1. **NO PLACEHOLDER CODE**: Every implementation must be production-ready. NEVER write "TODO", "in a real implementation", or similar placeholders unless explicitly requested by the user.
2. **CROSS-DIRECTORY TASKS**: Group related changes across directories into single tasks to ensure consistency. Never create isolated changes that require follow-up work in sibling directories.
3. **COMPLETE IMPLEMENTATIONS**: Each task must fully implement its feature including all consumers, type updates, and integration points.
4. **DETAILED SPECIFICATIONS**: Each task must include EXACTLY what to implement, including specific functions, types, and integration points to avoid "breaking change" confusion.
5. **CONTEXT AWARENESS**: Each task is part of a larger system - specify how it connects to other parts.
6. **MAKE BREAKING CHANGES**: Unless explicitly requested by the user, you MUST make breaking changes.

### Visual Dependency Tree
```
anchor/
├── spec_tests/
│   ├── debug_go_ssz_extraction.go (Task #0: Go SSZ debugging extraction)
│   ├── src/qbft/
│   │   ├── hash_debugging.rs (Task #1: Rust tree hash debugging tools)
│   │   ├── field_comparison.rs (Task #2: Field-level hash comparison)
│   │   └── create_message.rs (Task #4: Enhanced test validation with hash debugging)
│   │
│   └── ssv-spec/
│       └── debug_tree_hash.go (Task #3: Go tree hash step extraction)
```

### Execution Plan

#### Group A: Foundation - Go SSZ Extraction (Execute first)
- [ ] **Task #0**: Create Go SSZ debugging and tree hash extraction tools
  - **Folder**: `anchor/spec_tests/`
  - **File**: `debug_go_ssz_extraction.go`
  - **Imports**:
    - `"github.com/ssvlabs/ssv-spec/types"`
    - `"github.com/ssvlabs/ssv-spec/qbft"`
    - `"encoding/hex"`
    - `"encoding/json"`
    - `"fmt"`
    - `"os"`
    - `"crypto/sha256"`
  - **Implements**:
    - **Function `ExtractGoTreeHashSteps(msg *types.SignedSSVMessage) *TreeHashDebugInfo`**:
      - Extracts tree hash computation at each step
      - Captures individual field tree hashes before merkleization
      - Records intermediate merkleization steps
      - Returns complete tree hash computation trace
    - **Function `GetFieldTreeHashes(msg *types.SignedSSVMessage) map[string]string`**:
      - Computes tree hash for each field individually
      - Maps field names to their tree hash hex values
      - Includes nested field analysis for complex structures
      - Handles variable-length fields properly
    - **Function `DebugSSZStructure(msg *types.SignedSSVMessage) *SSZStructureInfo`**:
      - Analyzes SSZ structure before tree hashing
      - Extracts field offsets and sizes
      - Maps field boundaries and alignment
      - Validates SSZ container structure
    - **Function `TraceMerkleization(fields [][]byte) *MerkleTrace`**:
      - Steps through merkleization process
      - Records each hash computation step
      - Captures intermediate tree nodes
      - Provides complete merkle tree construction trace
    - **Function `ExtractTestCaseData(testName string) error`**:
      - Loads specific test case
      - Extracts Go SSZ data and tree hash steps
      - Saves debugging information to JSON files
      - Provides data for Rust comparison
    - **Struct `TreeHashDebugInfo`**: Complete tree hash computation trace
    - **Struct `SSZStructureInfo`**: SSZ structure analysis results
    - **Struct `MerkleTrace`**: Step-by-step merkleization trace
    - **Function `main()`**: CLI interface to extract debugging data for specific test cases
  - **Integration**: Standalone Go tool to extract debugging data for comparison with Rust
  - **Testing**: Verify extraction works for all failing test cases
  - **Output**: JSON files with complete Go tree hash computation traces

#### Group B: Rust Tree Hash Debugging (Execute after Group A)
- [ ] **Task #1**: Implement comprehensive Rust tree hash debugging tools
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `hash_debugging.rs`
  - **Imports**:
    - `use ssv_types::message::SignedSSVMessage;`
    - `use tree_hash::{TreeHash, TreeHashType};`
    - `use ssz_types::VariableList;`
    - `use serde::{Deserialize, Serialize};`
    - `use std::collections::HashMap;`
    - `use hex;`
  - **Implements**:
    - **Function `trace_rust_tree_hash(msg: &SignedSSVMessage) -> RustTreeHashTrace`**:
      - Traces Rust tree hash computation step by step
      - Captures individual field tree hashes
      - Records merkleization process
      - Provides complete computation trace
    - **Function `extract_field_tree_hashes(msg: &SignedSSVMessage) -> HashMap<String, String>`**:
      - Computes tree hash for each field individually
      - Maps field names to tree hash hex values
      - Handles nested structures and variable lists
      - Provides field-level debugging information
    - **Function `debug_merkleization_process(fields: &[Vec<u8>]) -> MerkleizationTrace`**:
      - Steps through Rust merkleization algorithm
      - Records each hash computation
      - Captures intermediate tree nodes
      - Compares with expected merkle tree structure
    - **Function `analyze_tree_hash_differences(rust_trace: &RustTreeHashTrace, go_data: &GoTreeHashData) -> HashDifferenceAnalysis`**:
      - Compares Rust and Go tree hash computation
      - Identifies exact point of divergence
      - Analyzes field-level differences
      - Provides root cause analysis
    - **Function `validate_field_correctness(msg: &SignedSSVMessage) -> FieldValidationResult`**:
      - Validates that field values match expected test data
      - Confirms field structure before tree hashing
      - Identifies any field-level issues
      - Provides comprehensive field validation
    - **Function `load_go_debug_data(test_name: &str) -> Option<GoTreeHashData>`**:
      - Loads Go debugging data from JSON files
      - Parses tree hash computation traces
      - Provides data for comparison with Rust
      - Handles missing or malformed data gracefully
    - **Struct `RustTreeHashTrace`**: Complete Rust tree hash computation trace
    - **Struct `GoTreeHashData`**: Parsed Go debugging data
    - **Struct `HashDifferenceAnalysis`**: Analysis of tree hash differences
    - **Struct `MerkleizationTrace`**: Rust merkleization process trace
    - **Struct `FieldValidationResult`**: Field validation results
  - **Integration**: Core debugging tools used by test framework and field comparison
  - **Testing**: Verify tracing works for all test cases and provides actionable debugging information

#### Group C: Field-Level Comparison (Execute after Group B)
- [ ] **Task #2**: Implement systematic field-level hash comparison
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `field_comparison.rs`
  - **Imports**:
    - `use super::hash_debugging::*;`
    - `use ssv_types::message::SignedSSVMessage;`
    - `use tree_hash::TreeHash;`
    - `use std::collections::HashMap;`
    - `use serde::{Deserialize, Serialize};`
  - **Implements**:
    - **Function `compare_field_tree_hashes(rust_msg: &SignedSSVMessage, test_name: &str) -> FieldComparisonResult`**:
      - Compares each field's tree hash between Rust and Go
      - Identifies which specific fields have different hashes
      - Provides byte-level analysis of field differences
      - Returns comprehensive comparison results
    - **Function `analyze_individual_field_differences(field_name: &str, rust_hash: &str, go_hash: &str, rust_msg: &SignedSSVMessage) -> FieldDifferenceAnalysis`**:
      - Deep analysis of individual field hash differences
      - Examines field structure and content
      - Identifies potential causes of hash mismatch
      - Provides specific recommendations for fixing
    - **Function `validate_nested_field_hashes(msg: &SignedSSVMessage, go_data: &GoTreeHashData) -> NestedFieldValidation`**:
      - Validates tree hashes for nested structures
      - Compares justification arrays field by field
      - Analyzes variable-length field handling
      - Identifies nested structure issues
    - **Function `test_field_isolation(msg: &SignedSSVMessage) -> FieldIsolationResult`**:
      - Tests each field in isolation
      - Creates minimal test cases for individual fields
      - Validates that isolated fields hash correctly
      - Identifies field interaction issues
    - **Function `compare_merkleization_algorithms(rust_fields: &[Vec<u8>], go_fields: &[Vec<u8>]) -> MerkleizationComparison`**:
      - Compares merkleization algorithms step by step
      - Identifies differences in tree construction
      - Analyzes padding and alignment differences
      - Provides algorithm-level comparison results
    - **Function `generate_field_fix_recommendations(comparison: &FieldComparisonResult) -> Vec<FixRecommendation>`**:
      - Generates specific recommendations for fixing field hash issues
      - Prioritizes fixes based on impact and complexity
      - Provides code examples for implementing fixes
      - Includes validation steps for each recommendation
    - **Struct `FieldComparisonResult`**: Comprehensive field comparison results
    - **Struct `FieldDifferenceAnalysis`**: Analysis of individual field differences
    - **Struct `NestedFieldValidation`**: Nested field validation results
    - **Struct `FieldIsolationResult`**: Field isolation test results
    - **Struct `MerkleizationComparison`**: Algorithm comparison results
    - **Struct `FixRecommendation`**: Specific fix recommendations
  - **Integration**: Used by test framework to identify and fix field-level issues
  - **Testing**: Verify field comparison works for all test cases and provides actionable results

#### Group D: Go Tree Hash Step Extraction (Execute in parallel with Group C)
- [ ] **Task #3**: Implement detailed Go tree hash step extraction
  - **Folder**: `anchor/spec_tests/ssv-spec/`
  - **File**: `debug_tree_hash.go`
  - **Imports**:
    - `"github.com/ssvlabs/ssv-spec/types"`
    - `"github.com/ssvlabs/ssv-spec/qbft"`
    - `"github.com/ferranbt/fastssz"`
    - `"crypto/sha256"`
    - `"encoding/hex"`
    - `"encoding/json"`
    - `"fmt"`
    - `"os"`
  - **Implements**:
    - **Function `ExtractDetailedTreeHashSteps(msg *types.SignedSSVMessage) (*DetailedTreeHashTrace, error)`**:
      - Extracts complete tree hash computation trace
      - Records each merkleization step with intermediate values
      - Captures field-level tree hashes before merkleization
      - Provides detailed debugging information
    - **Function `AnalyzeFastSSZBehavior(msg *types.SignedSSVMessage) (*FastSSZAnalysis, error)`**:
      - Analyzes fastssz implementation behavior
      - Identifies specific fastssz algorithms used
      - Compares with standard SSZ merkleization
      - Provides implementation-specific insights
    - **Function `ExtractMerkleTreeStructure(msg *types.SignedSSVMessage) (*MerkleTreeStructure, error)`**:
      - Extracts complete merkle tree structure
      - Maps tree nodes to their hash values
      - Provides tree traversal information
      - Enables comparison with Rust tree structure
    - **Function `TraceHashComputationPath(msg *types.SignedSSVMessage) (*HashComputationPath, error)`**:
      - Traces the exact path of hash computation
      - Records each hash operation and its inputs
      - Provides step-by-step computation trace
      - Enables exact replication in Rust
    - **Function `GenerateRustComparisonData(testName string, msg *types.SignedSSVMessage) error`**:
      - Generates comprehensive data for Rust comparison
      - Saves detailed tree hash traces to JSON
      - Provides structured data for automated comparison
      - Includes all necessary information for debugging
    - **Function `ValidateGoTreeHashConsistency(msg *types.SignedSSVMessage) error`**:
      - Validates internal consistency of Go tree hash computation
      - Checks for any anomalies in Go implementation
      - Ensures reliable baseline for comparison
      - Identifies potential Go-side issues
    - **Struct `DetailedTreeHashTrace`**: Complete tree hash computation trace
    - **Struct `FastSSZAnalysis`**: fastssz implementation analysis
    - **Struct `MerkleTreeStructure`**: Merkle tree structure information
    - **Struct `HashComputationPath`**: Hash computation path trace
    - **Function `main()`**: CLI interface for extracting detailed debugging data
  - **Integration**: Provides detailed Go debugging data for comparison with Rust
  - **Testing**: Verify extraction provides complete and accurate debugging information
  - **Output**: Comprehensive JSON files with detailed tree hash computation traces

#### Group E: Enhanced Test Validation (Execute after Groups A-D)
- [ ] **Task #4**: Enhance test validation with comprehensive hash debugging
  - **Folder**: `anchor/spec_tests/src/qbft/`
  - **File**: `create_message.rs`
  - **Imports**:
    - `use super::hash_debugging::*;`
    - `use super::field_comparison::*;`
    - `use std::process::Command;`
    - `use serde_json;`
  - **Implements**:
    - **Update `run` method in `CreateMessageTest`**:
      ```rust
      fn run(&self) -> bool {
          // Generate Go debugging data first
          self.generate_go_debug_data();
          
          // Create Rust message
          let rust_msg = self.create_rust_message();
          
          // Perform comprehensive hash debugging
          let hash_comparison = self.perform_hash_debugging(&rust_msg);
          
          // Compare tree hashes with detailed analysis
          let tree_hash_matches = self.compare_tree_hashes(&rust_msg, &hash_comparison);
          
          // Provide detailed debugging output
          self.output_debugging_results(&hash_comparison);
          
          tree_hash_matches
      }
      ```
    - **Function `generate_go_debug_data(&self) -> Result<(), String>`**:
      - Executes Go debugging tools to extract tree hash data
      - Saves debugging information for the specific test case
      - Ensures Go data is available for comparison
      - Handles Go tool execution errors gracefully
    - **Function `perform_hash_debugging(&self, rust_msg: &SignedSSVMessage) -> HashDebuggingResult`**:
      - Performs comprehensive hash debugging on Rust message
      - Compares with Go debugging data
      - Identifies exact point of hash divergence
      - Provides detailed analysis of differences
    - **Function `compare_tree_hashes(&self, rust_msg: &SignedSSVMessage, debug_result: &HashDebuggingResult) -> bool`**:
      - Compares final tree hashes between Rust and Go
      - Validates that debugging correctly identifies issues
      - Provides pass/fail determination for test
      - Includes detailed comparison logging
    - **Function `output_debugging_results(&self, debug_result: &HashDebuggingResult)`**:
      - Outputs comprehensive debugging information
      - Provides actionable recommendations for fixing issues
      - Includes field-level analysis and fix suggestions
      - Formats output for easy analysis
    - **Function `validate_field_correctness(&self, rust_msg: &SignedSSVMessage) -> FieldValidationSummary`**:
      - Validates that all fields contain expected values
      - Confirms field structure matches test expectations
      - Identifies any field-level issues before hash comparison
      - Provides comprehensive field validation summary
    - **Function `analyze_root_cause(&self, debug_result: &HashDebuggingResult) -> RootCauseAnalysis`**:
      - Analyzes debugging results to identify root cause
      - Provides specific recommendations for fixing the issue
      - Prioritizes potential fixes based on likelihood
      - Generates actionable debugging information
    - **Struct `HashDebuggingResult`**: Complete hash debugging results
    - **Struct `FieldValidationSummary`**: Field validation summary
    - **Struct `RootCauseAnalysis`**: Root cause analysis results
  - **Integration**: Enhanced test validation with comprehensive debugging for all create message tests
  - **Testing**: Verify debugging provides actionable information for all failing test cases
  - **Output**: Detailed debugging information that identifies exact root cause of hash mismatches

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
- Run debugging tools after each task to verify progress
- Focus on identifying exact root cause rather than implementing fixes

### Debugging Strategy
- Execute Go and Rust debugging tools in parallel
- Compare results at each level of tree hash computation
- Identify the exact point where hash calculations diverge
- Provide actionable analysis of root cause
- Test multiple theories simultaneously to accelerate debugging

### Progress Tracking
The checkboxes above represent the authoritative status of each task. Keep them updated as you work.

### Expected Debugging Outcomes
1. **Exact Root Cause Identification**: Pinpoint the specific step where tree hash computation diverges
2. **Field-Level Analysis**: Identify which fields (if any) have different tree hashes
3. **Algorithm Comparison**: Understand differences between `fastssz` and `tree_hash` implementations
4. **Actionable Fix Plan**: Provide specific recommendations for fixing the hash mismatch issues
5. **Comprehensive Documentation**: Document all findings for future reference and similar issues