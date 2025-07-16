# Runner Construction Tests Analysis

## 1. File Discovery

The following runner construction test files were found in the directory:

- `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/ssv-spec/ssv/spectest/generate/tests/runnerconstruction.RunnerConstructionSpecTest_RunnerConstruction_no_shares.json`
- `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/ssv-spec/ssv/spectest/generate/tests/runnerconstruction.RunnerConstructionSpecTest_RunnerConstruction_one_share.json`
- `/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests/ssv-spec/ssv/spectest/generate/tests/runnerconstruction.RunnerConstructionSpecTest_RunnerConstruction_many_shares.json`

## 2. Construction Process

### 2.1 Overview
Runners are constructed through the `ConstructBaseRunnerWithShareMap` function, which creates different types of runners based on the specified role. The construction process involves:

1. **Role-based Constructor Selection**: Different runner types are created based on the `RunnerRole`:
   - `RoleCommittee` → `CommitteeRunner`
   - `RoleProposer` → `ProposerRunner`
   - `RoleAggregator` → `AggregatorRunner`
   - `RoleSyncCommitteeContribution` → `SyncCommitteeAggregatorRunner`
   - `RoleValidatorRegistration` → `ValidatorRegistrationRunner`
   - `RoleVoluntaryExit` → `VoluntaryExitRunner`

2. **Component Initialization**: Each runner requires:
   - Message identifier based on role and validator/committee ID
   - Network interface for communication
   - Operator signer for message signing
   - Value check function for consensus validation
   - QBFT controller for consensus coordination

### 2.2 Base Runner Setup
All runners inherit from `BaseRunner` which contains:
- `RunnerRoleType`: The specific role type
- `BeaconNetwork`: Network configuration
- `Share`: Map of validator shares
- `QBFTController`: Consensus controller
- `highestDecidedSlot`: Tracking mechanism

## 3. Configuration Requirements

### 3.1 Share Structure
Each share contains:
- `ValidatorIndex`: Unique validator identifier
- `ValidatorPubKey`: 48-byte validator public key
- `SharePubKey`: 48-byte share-specific public key
- `Committee`: Array of committee members (max 13)
- `DomainType`: 4-byte domain identifier
- `FeeRecipientAddress`: 20-byte fee recipient address
- `Graffiti`: 32-byte graffiti data

### 3.2 Committee Structure
Each committee member includes:
- `SharePubKey`: 48-byte share public key
- `Signer`: Operator ID for the committee member

### 3.3 Network Configuration
- Beacon network type (`BeaconTestNetwork`)
- QBFT controller configuration
- Operator signing capabilities
- Value check functions specific to each role

## 4. State Management

### 4.1 Runner State Components
Runners maintain state through:
- `StartingDuty`: Current duty being executed
- `DecidedValue`: Consensus-decided value
- `PreConsensusContainer`: Pre-consensus signature container
- `PostConsensusContainer`: Post-consensus signature container
- `Finished`: Completion flag

### 4.2 Consensus State
- Height tracking for QBFT consensus
- Round management
- Message processing state
- Quorum achievement tracking

### 4.3 Committee-Specific State
The `CommitteeRunner` maintains additional state:
- `submittedDuties`: Track submitted duties by role and validator
- Supports multiple validators per duty
- Handles both attestation and sync committee duties

## 5. Failure Modes

### 5.1 Share Count Validation
Different runners have different share requirements:

#### No Shares (Empty Share Map)
- **Committee Runner**: Returns `"no shares"` error
- **All Other Runners**: Return `"must have one share"` error

#### One Share
- **All Runners**: Successfully construct (no errors)
- This is the valid scenario for single-validator runners

#### Many Shares (Multiple Validators)
- **Committee Runner**: Successfully constructs (no error)
- **All Other Runners**: Return `"must have one share"` error

### 5.2 Role-Specific Constraints
The validation logic shows:
- `RoleCommittee`: Can handle multiple shares (committee of validators)
- `RoleProposer`: Must have exactly one share
- `RoleAggregator`: Must have exactly one share  
- `RoleSyncCommitteeContribution`: Must have exactly one share
- `RoleValidatorRegistration`: Must have exactly one share
- `RoleVoluntaryExit`: Must have exactly one share

### 5.3 Construction Validation
The test framework validates:
- Expected error messages match actual errors
- Successful construction when no errors expected
- Proper error handling for invalid configurations

## 6. Share Management

### 6.1 Single Share Runners
For roles requiring exactly one share:
- Validation: `if len(share) != 1 { return nil, errors.New("must have one share") }`
- Access: `GetShare()` method returns the single share
- Usage: Direct validator operations on the single share

### 6.2 Multi-Share Committee Runner
For committee operations:
- Validation: `if len(share) == 0 { return nil, errors.New("no shares") }`
- Access: Iterates through share map by validator index
- Usage: Supports multiple validators in a single committee duty

### 6.3 Share Access Patterns
- **Single Share**: `GetShare()` returns the only share
- **Multiple Shares**: Access by validator index from share map
- **Committee Operations**: Iterate through all shares for duty execution

### 6.4 Key Management
Each share manages:
- Individual validator keys
- Committee member keys
- Operator signing keys
- Domain-specific configurations

## Key Findings Summary

1. **Role-Based Architecture**: Runner construction is strictly role-based with specific share requirements
2. **Validation Strategy**: Share count validation prevents misconfiguration at construction time
3. **Committee Flexibility**: Only committee runners support multiple validators
4. **Error Handling**: Clear error messages distinguish between "no shares" and "must have one share"
5. **State Isolation**: Each runner maintains independent state management
6. **Consensus Integration**: All runners integrate with QBFT consensus mechanism
7. **Type Safety**: Strong typing ensures proper configuration matching runner capabilities

The test suite effectively validates that runners can only be constructed with appropriate share configurations, preventing runtime errors and ensuring proper validator duty execution.