# QBFT Controller Test Implementation - Complete Context

## Current Task
We are implementing QBFT controller tests in Rust to match the Go ssv-spec implementation. The tests verify consensus message processing by replaying pre-signed messages through a controller that manages multiple QBFT instances.

## Critical Understanding
**THE KEY INSIGHT**: The Go tests DO NOT simulate actual consensus. They:
1. Create one controller (operator 1)
2. Feed it PRE-SIGNED messages from multiple operators (1,2,3,4)
3. Process messages to detect decisions (commit messages with quorum)
4. The decided value is already in the message's `full_data` field

## Current Working Directory
```bash
cd /home/dsfreakdude/code/sigp/anchor-qbft/anchor/spec_tests
```

## How to Build and Test
```bash
# Build the library
cargo build --lib

# Run all controller tests
cargo test test_qbft_controller

# Run a specific test
export TEST_FILTER="controller_valid.json"
cargo test test_qbft_controller -- --nocapture

# Run with debug output
export RUST_LOG=debug
cargo test test_qbft_controller -- --nocapture --test-threads=1

# Test specific patterns
export TEST_FILTER="controller_decide_no_quorum"
cargo test test_qbft_controller
```

## Architecture Mismatch - THE CORE PROBLEM

### Go Architecture (Test-Oriented)
- **Passive State Machines**: Instances only react to messages
- `Controller` maintains multiple instances at different heights
- `ProcessMsg` synchronously processes messages and returns
- No background tasks, no timers, just message processing
- Can create instances on-demand for decided messages

### Rust Architecture (Production-Oriented)
- **Active Async Tasks**: Instances run consensus independently
- `QbftManager` spawns background tasks that propose and timeout
- Designed for real consensus with multiple operators
- Instances keep running until they reach consensus

### Our Solution
We're building a controller adapter that:
1. Uses the real `QbftManager` for message processing
2. Tracks spawned tasks and aborts them on drop (prevents hanging)
3. Implements the Go controller's routing logic
4. Manages multiple instances across different heights

## File Structure

### Files We're Modifying
```
/home/dsfreakdude/code/sigp/anchor-qbft/anchor/spec_tests/
├── src/
│   ├── qbft/
│   │   ├── adapters/
│   │   │   └── manager.rs         # Main adapter implementation (PRIMARY WORK HERE)
│   │   └── controller_test.rs     # Test runner
│   └── lib.rs                     # Test framework with TEST_FILTER support
└── ssv-spec/qbft/spectest/generate/tests/  # Test JSON files
```

### Key External Dependencies
- `/home/dsfreakdude/code/sigp/anchor-qbft/anchor/qbft_manager/` - Production QBFT manager
- `/home/dsfreakdude/code/sigp/anchor-qbft/anchor/common/qbft/` - Core QBFT types
- `/home/dsfreakdude/code/sigp/anchor-qbft/anchor/common/ssv_types/` - SSV message types

## Current Implementation Status

### What's Working ✅
1. **Basic message processing** - Can process pre-signed messages
2. **Decision detection** - Correctly identifies commit + quorum as decision
3. **Instance cleanup** - Spawned tasks are aborted on drop (no hanging)
4. **Error handling** - Tests expecting errors now pass
5. **Simple tests pass** - `controller_valid.json`, `controller_decide_no_quorum.json`

### What's NOT Working ❌
1. **Multiple instance management** - Only tracks one height properly
2. **"instance not found" errors** - Not detecting missing instances
3. **Future/past message handling** - No height-based routing
4. **Message validation** - Missing hash checks (H(data) != root)
5. **Complex tests fail** - Multi-instance, future messages, validation errors

## Deep Research Findings

### Test Categories and Their Requirements

#### 1. "no_instance" Tests
- **Example**: `controller_no_instance_running.json`
- **Expects**: "instance not found" error
- **Problem**: We start an instance even when we shouldn't
- **Solution**: Check if instance exists before processing

#### 2. "multi" Tests  
- **Example**: `controller_multi_decide_instances.json`
- **Expects**: Multiple instances at different heights
- **Problem**: We only track current_height
- **Solution**: Proper instance container

#### 3. "future" Tests
- **Example**: `controller_decide_future_instance.json`
- **Expects**: Future messages rejected or handled specially
- **Problem**: No future detection
- **Solution**: Compare message height with controller height

#### 4. "invalid" Tests
- **Example**: `controller_decide_invalid_value.json`
- **Expects**: Validation errors like "H(data) != root"
- **Problem**: No comprehensive validation
- **Solution**: Add hash and signature validation

## The 18-Step Implementation Plan

### CURRENT PROGRESS: Step 7 of 18

#### Completed Steps ✅
1. **Step 1**: Add InstanceState enum to track instance states
2. **Step 2**: Replace decided_instances HashMap with stored_instances  
3. **Step 3**: Add has_instance() method to check if instance exists
4. **Step 4**: Add is_future_message() method for future detection
5. **Step 5**: Add is_decided_message() as standalone check
6. **Step 6**: Extract decode_qbft_message() helper method

#### Current Step 🔄
7. **Step 7**: Implement upon_decided() for decided messages

#### Remaining Steps 📋
8. **Step 8**: Add validate_decided() for message validation
9. **Step 9**: Refactor process_msg() to use routing logic
10. **Step 10**: Handle 'instance not found' error
11. **Step 11**: Support creating instances for past heights
12. **Step 12**: Add hash validation (H(data) != root)
13. **Step 13**: Update height on future decided messages
14. **Step 14**: Test with no_instance_running test
15. **Step 15**: Test with future message tests
16. **Step 16**: Test with multi instance tests
17. **Step 17**: Test with validation error tests
18. **Step 18**: Run all controller tests

## Current Code State

### ControllerAdapter Structure (manager.rs)
```rust
// Line 17-23: InstanceState enum
enum InstanceState {
    Active,           // Instance is running
    Decided(Vec<u8>), // Instance decided with value
}

// Line 42-45: Instance tracking
current_height: InstanceHeight,
stored_instances: HashMap<InstanceHeight, InstanceState>, // All instances
current_committee_size: usize,
instance_handles: Vec<tokio::task::JoinHandle<()>>, // For cleanup

// Lines 218-231: Helper methods added
fn has_instance(&self, height: InstanceHeight) -> bool
fn is_future_message(&self, height: InstanceHeight) -> bool
fn calculate_quorum(&self, committee_size: usize) -> usize
fn is_decided_message(&self, qbft_msg: &QbftMessage, operator_ids: &[OperatorId]) -> bool
fn extract_decided_value(signed_msg: &SignedSSVMessage) -> Vec<u8>
fn decode_qbft_message(test_msg: &TestSignedSSVMessage) -> Result<(SignedSSVMessage, QbftMessage), String>

// Lines 286-292: Drop implementation for cleanup
impl Drop for ControllerAdapter {
    fn drop(&mut self) {
        for handle in self.instance_handles.drain(..) {
            handle.abort();
        }
    }
}
```

### Key Changes Made
1. Replaced `decided_instances: HashMap<Height, Vec<u8>>` with `stored_instances: HashMap<Height, InstanceState>`
2. Added instance state tracking (Active vs Decided)
3. Added helper methods for routing logic
4. Implemented Drop trait to abort spawned tasks

## Next Implementation Steps (Detailed)

### Step 7: Implement upon_decided() (CURRENT)
Add around line 280 in manager.rs:
```rust
async fn upon_decided(&mut self, test_msg: &TestSignedSSVMessage) -> Result<Option<Vec<u8>>, String> {
    let (signed_msg, qbft_msg) = Self::decode_qbft_message(test_msg)?;
    let height = InstanceHeight::from(qbft_msg.height as usize);
    
    // TODO: Add validation (Step 8)
    
    // Extract decided value
    let decided_value = Self::extract_decided_value(&signed_msg);
    
    // Update controller height if this is a future decided
    if *height > *self.current_height {
        self.current_height = height;
    }
    
    // Store or create instance as decided
    self.stored_instances.insert(height, InstanceState::Decided(decided_value.clone()));
    
    // Process through manager if instance exists
    if self.has_instance(height) {
        self.manager.receive_data(signed_msg, qbft_msg)
            .map_err(|e| format!("Failed to process decided: {:?}", e))?;
    }
    
    Ok(Some(decided_value))
}
```

### Step 9: Refactor process_msg() with routing
Replace current process_msg implementation with:
```rust
pub async fn process_msg(&mut self, test_msg: &TestSignedSSVMessage) -> Result<Option<Vec<u8>>, String> {
    let (signed_msg, qbft_msg) = Self::decode_qbft_message(test_msg)?;
    let height = InstanceHeight::from(qbft_msg.height as usize);
    let operator_ids = signed_msg.operator_ids();
    
    // 1. Check if decided
    if self.is_decided_message(&qbft_msg, operator_ids) {
        return self.upon_decided(test_msg).await;
    }
    
    // 2. Check if future
    if self.is_future_message(height) {
        return Err("future msg from height, could not process".into());
    }
    
    // 3. Check if instance exists
    if !self.has_instance(height) {
        return Err("instance not found".into());
    }
    
    // 4. Process normally
    self.manager.receive_data(signed_msg, qbft_msg)
        .map_err(|e| format!("Failed to process: {:?}", e))?;
    
    Ok(None)
}
```

## Go Reference Implementation

### Key Go Files to Reference
- `ssv-spec/qbft/controller.go` - Main controller logic
- `ssv-spec/qbft/spectest/tests/controller_spectest.go` - Test runner
- `ssv-spec/qbft/decided.go` - Decided message handling

### Go's ProcessMsg Flow
```go
func (c *Controller) ProcessMsg(signedMessage *types.SignedSSVMessage) (*types.SignedSSVMessage, error) {
    // 1. Validate message
    if err := c.BaseMsgValidation(msg); err != nil {
        return nil, errors.Wrap(err, "invalid msg")
    }
    
    // 2. Check if decided
    if isDecided {
        return c.UponDecided(msg)
    }
    
    // 3. Check if future
    if isFuture {
        return nil, fmt.Errorf("future msg from height, could not process")
    }
    
    // 4. Process for existing instance
    return c.UponExistingInstanceMsg(msg)
}
```

## Test Data Format
```json
{
  "RunInstanceData": [{
    "InputValue": "AQIDBA==",           // Base64 value to start instance
    "InputMessages": [{                 // Pre-signed messages to process
      "SSVMessage": {
        "MsgType": 0,
        "Data": "..."                    // SSZ-encoded QbftMessage
      },
      "OperatorIDs": [1,2,3],           // Which operators signed
      "Signatures": ["..."],            
      "FullData": "..."                 // Decided value (for commit msgs)
    }],
    "ExpectedDecidedState": {
      "DecidedCnt": 1,                  // How many decisions expected
      "DecidedVal": "..."               // Expected decided value
    },
    "ControllerPostRoot": "0x..."       // Expected state hash (Go-specific)
  }],
  "ExpectedError": ""                   // Expected error message if any
}
```

## Common Issues and Solutions

### Issue: Tests hang forever
**Cause**: Instance keeps running waiting for consensus
**Solution**: Implemented Drop trait to abort spawned tasks

### Issue: "instance not found" error not thrown
**Cause**: Not checking if instance exists before processing
**Solution**: Add has_instance() check in routing logic (Step 10)

### Issue: Future messages not handled
**Cause**: No height comparison logic
**Solution**: Added is_future_message() method (Step 4)

### Issue: Validation errors not detected
**Cause**: No hash/signature validation
**Solution**: Need to implement validate_decided() (Step 8)

## Environment Variables
- `TEST_FILTER` - Filter to specific test files
- `RUST_LOG` - Set to "debug" for detailed output

## Git Information
- Current branch: `impl-qbft-spec-tests`
- Main branch: `stable`
- Modified files:
  - `src/qbft/adapters/manager.rs`
  - `src/qbft/controller_test.rs`
  - `src/lib.rs`

## Critical Functions to Remember

### Quorum Calculation
```rust
quorum = n - f where f = (n-1)/3
// For 4 operators: quorum = 4 - 1 = 3
```

### Decision Detection
```rust
is_decided = is_commit_message && has_quorum_signatures
```

### Instance Height vs Controller Height
- Instance height: Height of a specific QBFT instance
- Controller height: Current height the controller is at
- Can have instances at different heights simultaneously

## Instructions to Continue

When resuming work:
1. Read this file completely
2. Check current step in the 18-step plan (Step 7: upon_decided)
3. Run `cargo build --lib` to ensure everything compiles
4. Implement the next step following the detailed instructions above
5. Test frequently with specific test cases
6. Update this file with any new findings or changes

## Key Principles
1. **Don't spawn real consensus** - Just track states
2. **Route messages properly** - Decided → Future → Existing → Error
3. **Test frequently** - Run tests after each step
4. **Match Go behavior** - Reference Go implementation for logic
5. **Handle all error cases** - Expected errors should match Go's

## Notes from User (CLAUDE.md)
- Use `ast-grep --lang rust` for syntax-aware searches
- Do what has been asked; nothing more, nothing less
- NEVER create files unless absolutely necessary
- ALWAYS prefer editing existing files
- Build often instead of all at once

This is a spec test implementation - we're testing message processing, not running actual consensus!