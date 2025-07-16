# Task #19: Detailed SSV Test Implementation Strategy

## Overview
This document provides an extremely detailed strategy for implementing each SSV spec test category using our existing SpecTest trait infrastructure. The approach focuses on leveraging existing Rust SSV infrastructure without building complex orchestration layers, implementing focused test execution that validates outputs against expected JSON results.

## Core Implementation Approach

### Philosophy
- **Use Existing Infrastructure**: Leverage existing QBFT, message validation, signature collection without modification
- **Implement SpecTest Trait**: Each test category implements our existing SpecTest trait pattern  
- **Validate Outputs**: Compare actual execution results with expected JSON outputs
- **No Complex Orchestration**: Keep implementations focused and direct
- **Maintain Consistency**: Follow existing spec test patterns in the codebase

### Test Execution Pattern
All SSV tests follow this pattern:
1. **Parse JSON test data** using existing custom deserializers (already implemented)
2. **Execute test logic** using appropriate SSV infrastructure components
3. **Validate outputs** against expected results from JSON
4. **Return success/failure** based on specification compliance

## Detailed Implementation Strategy by Category

### 1. Multi Message Processing Tests (99 tests)
**Files**: `tests.MultiMsgProcessingSpecTest_*.json`
**Infrastructure**: QBFT consensus + message validation + signature collection

#### Test Structure Understanding
- Each test contains multiple sub-tests in `Tests` array
- Sub-tests must be executed sequentially 
- Each sub-test has: Runner config, Messages array, Expected outputs, Expected error

#### Execution Strategy
```
For each MultiMsgProcessingSpecTest file:
  For each sub-test in Tests array:
    1. Initialize QBFT instance with runner configuration
    2. Set up validator duty and committee duty if present
    3. Process each message in Messages array through QBFT sequentially
    4. If error occurs: validate error matches expected_error
    5. If no error: validate all outputs match expected values
      - PostDutyRunnerStateRoot
      - OutputMessages array
      - BeaconBroadcastedRoots array
```

#### Key Validation Points
- **Message Processing**: Each FlexibleSignedSSVMessage processed through QBFT consensus
- **State Transitions**: QBFT state root must match PostDutyRunnerStateRoot  
- **Output Generation**: Generated OutputMessages must match expected array exactly
- **Error Handling**: Expected errors must match actual QBFT/validation errors exactly
- **Beacon Roots**: BeaconBroadcastedRoots must match expected broadcast data

#### Infrastructure Integration
- **QBFT Manager**: Use existing QbftManager for consensus execution
- **Message Validator**: Use existing MessageValidator for message validation
- **Signature Collector**: Use existing SignatureCollector for partial signatures
- **FlexibleSignedSSVMessage**: Use existing custom deserializer for null/base64 handling

#### Success Criteria
- All 99 test files execute without parsing errors
- Each sub-test passes with outputs exactly matching expected JSON values
- Error conditions properly detected and validated against expected error messages

### 2. Single Message Processing Tests (3 tests)
**Files**: `tests.MsgProcessingSpecTest_*.json` 
**Infrastructure**: QBFT consensus + message validation + slashing detection

#### Test Structure Understanding  
- Direct test execution (no sub-tests)
- 7-message consensus sequences for complete QBFT rounds
- Focus on atomic behaviors: block proposal workflows, slashing detection

#### Execution Strategy
```
For each MsgProcessingSpecTest file:
  Based on test name:
    - "propose regular decide blinded": Test regular → blinded block transition
    - "propose blinded decide regular": Test blinded → regular block transition  
    - "decide on slashable attestation": Test slashing detection and rejection
    
  Execute 7-message sequence through QBFT:
    1. Initialize QBFT with appropriate role (proposer for block tests)
    2. Process messages 1-7 through consensus pipeline
    3. For block proposal tests: validate successful completion
    4. For slashing test: validate slashing detection and rejection
```

#### Key Validation Points
- **Block Type Transitions**: Validate seamless regular ↔ blinded block proposal transitions
- **7-Message Flow**: Complete QBFT consensus round (prepare → pre-prepare → commit)
- **Slashing Detection**: Validate slashing detection occurs before signature production
- **State Consistency**: Ensure state transitions work correctly with different block types
- **MEV-boost Compatibility**: Block proposal tests validate MEV-boost integration

#### Infrastructure Integration
- **QBFT Manager**: Use existing consensus implementation for 7-message sequences
- **Message Validator**: Use existing slashing detection for attestation validation
- **Block Processing**: Use existing block proposal infrastructure

#### Success Criteria
- Block proposal tests complete successfully with proper state transitions
- Slashing test properly detects and rejects slashable attestations
- No signatures produced for slashable data
- All outputs match expected JSON values

### 3. Validation Tests (13 tests)
**Files**: `valcheck.SpecTest_*.json` and `valcheck.MultiSpecTest_*.json`
**Infrastructure**: Message validation + duties tracking + slashing detection

#### Test Structure Understanding
- Single validation tests: Direct input → validation → result comparison
- Multi validation tests: Same validation across all 4 runner roles (0-3)
- Focus areas: Slashing detection, temporal constraints, duty validation

#### Execution Strategy
```
For SpecTest validation files:
  1. Parse input data (attestation data, consensus data, etc.)
  2. Run through existing MessageValidator validation pipeline
  3. Compare validation result with expected error/success
  
For MultiSpecTest validation files:
  For each runner role (0, 1, 2, 3):
    1. Parse duty data for specific role
    2. Run through DutiesTracker validation
    3. Compare result with expected error for that role
```

#### Key Validation Points
- **Slashing Prevention**: Validate slashable attestation detection across majority/minority scenarios
- **Temporal Constraints**: Validate epoch ordering (source < target) and future/past constraints  
- **Duty Validation**: Validate validator identity, role type, timing across all runner roles
- **Error Specificity**: Validate specific error messages match expected patterns exactly
- **Context Awareness**: Slot-specific slashing checks (slot 12 vs 13 scenarios)

#### Infrastructure Integration
- **MessageValidator**: Use existing 5-layer validation pipeline directly
- **DutiesTracker**: Use existing duty validation for multi-role tests
- **Slashing Detection**: Use existing slashing prevention logic

#### Success Criteria
- All validation rules properly detect invalid conditions
- Valid data passes validation without errors
- Error messages exactly match expected JSON error strings
- Multi-role tests validate consistently across all runner roles

### 4. Partial Signature Tests (5 tests)
**Files**: `partialsigcontainer.PartialSigContainerTest_*.json`
**Infrastructure**: Signature collection + BLS aggregation + quorum validation

#### Test Structure Understanding
- All tests use quorum=3 (3-of-4 threshold)
- Test scenarios: quorum success, duplicate handling, insufficient signatures, invalid signatures
- Single validator focus (ValidatorIndex "1")

#### Execution Strategy
```
For each PartialSigContainerTest file:
  1. Parse signature_msgs array into PartialSignatureMessage structs
  2. Use SignatureCollector to aggregate signatures with quorum=3
  3. Compare aggregation result with expected_result and expected_quorum
  4. Validate error message if aggregation fails
```

#### Key Validation Points
- **Threshold Signatures**: 3-of-4 quorum formation with BLS aggregation
- **Duplicate Handling**: Ensure duplicate signatures don't break aggregation (deduplicate by signer)
- **Cryptographic Validation**: Signatures must be cryptographically valid for aggregation
- **Error Conditions**: "could not reconstruct a valid signature" for insufficient/invalid signatures
- **Quorum Logic**: Only unique valid signatures count toward threshold

#### Infrastructure Integration
- **SignatureCollector**: Use existing threshold signature aggregation directly
- **BLS Aggregation**: Use existing BLS signature reconstruction
- **Validation Pipeline**: Use existing signature validation logic

#### Success Criteria
- Quorum tests succeed with valid aggregated signatures matching expected_result
- Duplicate tests succeed despite signature duplication
- Insufficient signature tests fail with proper error message
- Invalid signature tests fail cryptographic validation

### 5. Committee Tests (21 tests)
**Files**: `committee.CommitteeSpecTest_*.json` and `committee.MultiCommitteeSpecTest_*.json`
**Infrastructure**: QBFT consensus + committee coordination + duty execution

#### Test Structure Understanding
- 4-operator committee structure standard across all tests
- Support for attestation (Type 0) and sync committee (Type 3) duties
- Multi-committee tests support up to 30 concurrent duties

#### Execution Strategy
```
For CommitteeSpecTest files (2 tests):
  1. Parse committee configuration and duty
  2. Initialize Committee with 4-operator structure
  3. Execute duty through committee coordination
  4. Validate committee consensus and outputs
  
For MultiCommitteeSpecTest files (19 tests):
  For each sub-test:
    1. Set up committee with test configuration  
    2. Execute duty type (attestation or sync committee)
    3. Validate committee coordination and consensus
    4. Check error conditions or successful completion
```

#### Key Validation Points
- **Committee Coordination**: 4-operator committee must coordinate properly for duties
- **Duty Execution**: Attestation and sync committee duties executed through committee
- **Consensus Formation**: QBFT consensus within committee structure with 3-of-4 threshold
- **Error Handling**: Proper error handling for missing shares, invalid duties, timing issues
- **High Volume**: Support for concurrent duty processing (up to 30 duties)

#### Infrastructure Integration
- **Committee Management**: Use existing committee structure and coordination
- **QBFT Manager**: Use existing consensus with committee configuration
- **DutiesTracker**: Use existing duty execution infrastructure

#### Success Criteria
- Committee successfully coordinates duty execution
- Consensus reached within committee structure
- All duty types execute properly through committee
- Error conditions properly detected and handled

### 6. New Duty Tests (11 tests)
**Files**: `newduty.*.json`
**Infrastructure**: Duties tracking + duty lifecycle + timing validation

#### Test Structure Understanding
- All tests are multi-tests with various duty assignment scenarios
- Focus on duty initiation, timing validation, lifecycle management
- Support for different duty types (sync committee aggregator, attestation)

#### Execution Strategy
```
For each newduty test file:
  For each sub-test:
    1. Set up initial state if present (pre-existing duties, consensus state)
    2. Process new duty assignment through DutiesTracker
    3. Validate duty timing constraints (not past, not too far future)
    4. Execute duty through complete lifecycle:
       INITIALIZED → PRE_CONSENSUS → CONSENSUS → POST_CONSENSUS → COMPLETED
    5. Validate final state and duty completion
```

#### Key Validation Points
- **Duty Assignment**: New duties properly assigned and tracked by DutiesTracker
- **Timing Constraints**: Slot-based validation prevents past duties and far future duties
- **Lifecycle Management**: Complete duty execution through all state transitions
- **Multi-Duty Coordination**: Handle multiple concurrent duties of different types
- **State Consistency**: Maintain consistent state across duty lifecycle

#### Infrastructure Integration
- **DutiesTracker**: Use existing duty assignment and tracking logic
- **QBFT Manager**: Use existing consensus for duty execution phases
- **State Management**: Use existing state transition validation

#### Success Criteria
- Duties properly assigned without timing violations
- Complete lifecycle execution with proper state transitions
- Multi-duty scenarios handled correctly
- Error conditions properly detected (past duties, conflicts, etc.)

### 7. Runner Construction Tests (3 tests)
**Files**: `runnerconstruction.*.json`
**Infrastructure**: Runner initialization + share validation + role-based constraints

#### Test Structure Understanding
- 3 test scenarios: "no shares", "one share", "many shares"
- Each test has 6 sub-tests (one per runner role type)
- Role-based validation: Committee vs non-committee runners have different requirements

#### Execution Strategy
```
For each runnerconstruction test file:
  For each runner role (Committee, Proposer, Aggregator, SyncCommittee, ValidatorRegistration, VoluntaryExit):
    1. Parse shares configuration based on test scenario
    2. Attempt runner construction with role and shares
    3. Validate construction success/failure based on role requirements:
       - Committee: Can handle multiple shares (committee of validators)
       - Others: Must have exactly one share
    4. Validate error messages for construction failures
```

#### Key Validation Points
- **Role-Based Validation**: Committee runners can handle multiple shares, others need exactly one
- **Share Count Validation**: Proper validation of share requirements per role
- **Error Message Validation**: Specific error messages for different failure scenarios
- **Construction Success**: Validate successful construction for appropriate role/share combinations

#### Infrastructure Integration
- **Runner Construction**: Use existing runner initialization logic
- **Share Management**: Use existing validator share handling
- **Role Validation**: Use existing runner role constraint validation

#### Success Criteria
- "no shares": All roles fail with appropriate error messages
- "one share": Committee fails, others succeed
- "many shares": Committee succeeds, others fail
- Error messages match expected patterns exactly

### 8. Sync Committee Aggregator Tests (3 tests)
**Files**: `synccommitteeaggregator.*.json`
**Infrastructure**: Sync committee logic + aggregator selection + signature aggregation

#### Test Structure Understanding
- 3 test scenarios: "none selected", "all selected", "some selected"
- Multi-operator coordination (operators 1, 2, 3)
- 4-phase aggregation process with selection proofs

#### Execution Strategy
```
For each synccommitteeaggregator test file:
  1. Phase 1: Selection proof generation and validation
     - Determine which validators selected as aggregators
     - Handle none/all/some selection scenarios
  
  2. Phase 2: Consensus on contribution data (if selected)
     - Use QBFT for agreement on contribution
     - Coordinate between selected operators
  
  3. Phase 3: Signature reconstruction (if consensus reached)
     - Collect partial signatures from operators
     - Aggregate using threshold cryptography (2f+1)
  
  4. Phase 4: Beacon chain submission (if signatures valid)
     - Submit aggregated contribution to beacon node
     - Validate successful submission
```

#### Key Validation Points
- **Selection Process**: Proper aggregator selection based on VRF proofs
- **Consensus Coordination**: Multi-operator coordination with threshold signatures
- **Partial Aggregation**: Handle scenarios where only subset selected as aggregators
- **Graceful Termination**: Proper handling of "none selected" scenarios without errors
- **Threshold Cryptography**: Proper signature aggregation with 2f+1 threshold

#### Infrastructure Integration
- **Sync Committee Logic**: Use existing sync committee duty handling
- **Selection Proofs**: Use existing VRF and selection proof logic
- **Signature Aggregation**: Use existing threshold signature collection
- **Beacon Integration**: Use existing beacon node communication

#### Success Criteria
- "none selected": Graceful termination without errors
- "all selected": Successful full aggregation and submission
- "some selected": Successful partial aggregation with subset
- All phases execute properly with correct outputs

## Integration with Existing Infrastructure

### SpecTest Trait Implementation
Each test category implements the existing SpecTest trait:
```rust
impl SpecTest for SsvTestCategory {
    fn name(&self) -> &str { &self.name }
    fn setup(&mut self) { /* Initialize category-specific infrastructure */ }
    fn run(&self) -> bool { /* Execute test and validate outputs */ }
    fn test_type() -> SpecTestType { SpecTestType::Ssv(SsvSpecTestType::Category) }
}
```

### Test Discovery Integration
Extend existing test discovery to find SSV test files using filename patterns:
- `tests.MultiMsgProcessingSpecTest_*.json` → MultiMessageProcessing
- `tests.MsgProcessingSpecTest_*.json` → SingleMessageProcessing  
- `valcheck.*.json` → Validation
- `partialsigcontainer.PartialSigContainerTest_*.json` → PartialSignatures
- `committee.*.json` → Committee
- `newduty.*.json` → NewDuty
- `runnerconstruction.*.json` → RunnerConstruction
- `synccommitteeaggregator.*.json` → SyncCommitteeAggregator

### Output Validation Strategy
All tests validate outputs by comparing actual execution results with expected JSON values:
- **Exact String Matching**: For error messages, state roots, signature values
- **Array Comparison**: For OutputMessages, BeaconBroadcastedRoots with order preservation
- **Structural Comparison**: For complex objects with field-by-field validation
- **Null Handling**: Proper comparison of null vs empty vs populated values

### Error Handling Strategy
- **Expected Errors**: Compare actual error messages with expected_error strings
- **Unexpected Errors**: Fail test if error occurs when expected_error is empty
- **Error Specificity**: Validate exact error message content, not just error occurrence
- **Context Preservation**: Maintain error context for debugging failed tests

## Success Criteria Summary

### Per-Category Success Metrics
1. **Multi Message Processing**: 99/99 tests pass with exact output matching
2. **Single Message Processing**: 3/3 tests pass with proper behavior validation
3. **Validation**: 13/13 tests pass with correct validation logic
4. **Partial Signatures**: 5/5 tests pass with proper signature aggregation
5. **Committee**: 21/21 tests pass with committee coordination
6. **New Duty**: 11/11 tests pass with duty lifecycle management
7. **Runner Construction**: 3/3 tests pass with role-based validation
8. **Sync Committee Aggregator**: 3/3 tests pass with aggregation workflows

### Overall Success Metrics
- **100% Test Coverage**: All 157 SSV specification tests implemented
- **100% Pass Rate**: All tests pass with outputs matching expected JSON values
- **Infrastructure Integration**: All tests use existing Rust SSV infrastructure
- **Specification Compliance**: Perfect compatibility with Go reference implementation
- **Performance**: Sub-second execution for individual tests, full suite under 10 minutes

## Implementation Priority
1. **Start with Validation Tests**: Simplest category to validate approach
2. **Add Partial Signatures**: Build confidence with signature infrastructure
3. **Implement Single Message Processing**: Validate QBFT integration
4. **Add Multi Message Processing**: Most complex category with highest value
5. **Complete remaining categories**: Committee, NewDuty, RunnerConstruction, SyncCommitteeAggregator

This strategy provides the detailed roadmap for implementing all SSV specification tests using our existing infrastructure while maintaining specification compliance and achieving production-quality validation.