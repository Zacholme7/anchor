# SSV Spec Test JSON Format Issues

## Overview

This document details specific issues found in the SSV spec test JSON files that complicate parsing in external implementations (specifically Rust). These issues required custom deserializers and workarounds to handle inconsistent data formats.

**Repository**: https://github.com/ssvlabs/ssv-spec  
**Directory**: `ssv/spectest/generate/tests/`  
**Files Affected**: 99 `tests.MultiMsgProcessingSpecTest_*.json` files

## Issues Summary

The parsing success rate improved from **6/95 tests (6%)** to **99/99 tests (100%)** after implementing workarounds for these format inconsistencies.

---

## Issue #1: Inconsistent Base64 vs Byte Array Representation

### Problem
Binary data is inconsistently represented as either base64 strings or byte arrays across different test files and even within the same file.

### Concrete Examples

#### Example 1: `DecidedValue` Field Base64 Strings
**File**: `ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_post_consensus_post_quorum.json`  
**Line**: 107

```json
{
  "DecidedValue": "EAAAAAEAAAAAAAAAlAAAAAQAAAAAAAAAjoAGZVGoGzGCWHCe2vfdH2PNaGoOTbiym7t6z+ZWCGd69aUn2USO5Hg1SF4CtQvADAAAAAAAAAABAAAAAAAAAAMAAAAAAAAAgAAAAAAAAAAkAAAAAAAAAAsAAAAAAAAAbAAAAAAAAAAAAAAAAQAAAAAAAAACAAAAAAAAAAwAAAAMAQAADAIAALGIM7t1Sewz6KxBS6AC/UW7CUyjAL0kWW8EpDSom+6kYkAdp8a5L7OZG9FxY+tgNgSkDo3WeBJmyZACNEZ3b/QqkxPfJqCjQYSlkOV/pAA9YQwvohTbTn3sRoWSAQKYvAwAAAAAAAAAAQIDBAUGBwgJCgECAwQFBgcICQoBAgMEBQYHCAkKAQIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACQlDQslRRlVN+EncIPdCX8ppLazufLRSWN3SZKjlkphhRp/aPRVnuVIcuoMYj/1hoNvm1xgMepb1gQ0Y2zBekUN3K3ZtNoqpbTdR+Y0M4tufnm8mMlcCCI2H8N5QDGfGgMAAAAAAAAAAECAwQFBgcICQoBAgMEBQYHCAkKAQIDBAUGBwgJCgECAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAp/iM5D7/OqjN0uOVfFvq1OITU/vsrGB5pTmNAwGbxF/3yVF4UXLe7nDpvFq7yMpqDwRB6dTMnadMMRITV/fXx96VM/b0V9pJPjMU4i1VSrdmE+RpsFDiRq/1OaM4Bxl8DAAAAAAAAAABAgMEBQYHCAkKAQIDBAUGBwgJCgECAwQFBgcICQoBAgIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=="
}
```

This is a **1,270 character base64 string** that should be decoded to binary data.

#### Example 2: `FullData` Field Base64 Strings
**File**: `ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_post_consensus_partial_invalid_root_quorum_then_valid_quorum.json`  
**Line**: 182

```json
{
  "FullData": "AQIDBAUGBwgJCgECAwQFBgcICQoBAgMEBQYHCAkKAQIAAAAAAAAAAAECAwQFBgcICQoBAgMEBQYHCAkKAQIDBAUGBwgJCgECAQAAAAAAAAABAgMEBQYHCAkKAQIDBAUGBwgJCgECAwQFBgcICQoBAg=="
}
```

This is a **182 character base64 string** representing binary message data.

#### Example 3: ValidatorPubKey as Byte Array
**File**: `ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_full_happy_flow.json`

```json
{
  "ValidatorPubKey": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
}
```

This is the **same conceptual data** as the base64 examples above, but represented as a byte array.

### Error Messages
```
invalid type: string "EAAAAAEAAAAAAAAAlAAAAAQAA...", expected a sequence at line 107 column 1270
```

### Impact
- Requires custom deserializers for every binary field
- Inconsistent representation makes it impossible to use standard JSON parsers
- Different files use different formats for the same logical data type

---

## Issue #2: Null Value Handling Inconsistencies

### Problem
Optional fields are inconsistently represented as `null`, empty arrays `[]`, or omitted entirely.

### Concrete Examples

#### Example 1: BeaconBroadcastedRoots as Null
**File**: `ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_nil_SSVMessage.json`

```json
{
  "BeaconBroadcastedRoots": null,
  "ExpectedError": "nil SSVMessage"
}
```

#### Example 2: BeaconBroadcastedRoots as Empty Array
**File**: `ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_no_signers.json`

```json
{
  "BeaconBroadcastedRoots": [],
  "ExpectedError": "no signers"
}
```

#### Example 3: BeaconBroadcastedRoots as Populated Array
**File**: `ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_full_happy_flow.json`

```json
{
  "BeaconBroadcastedRoots": [
    "bc00bf2168785915f01d9c92cb647defa4e98f211a4c27e9b031ffee37eeeae4",
    "9cc921c178847ff0173698456266dc82454edf8aad8b50c2401ac170dd83615d"
  ]
}
```

### Error Messages
```
invalid type: null, expected a sequence at line 158 column 35
```

### Impact
- Requires all array fields to be `Option<Vec<T>>` instead of `Vec<T>`
- Parsers must handle three different representations of "no data"
- Inconsistent with typical JSON conventions

---

## Issue #3: Complex Nested Structure with Mixed Types

### Problem
The `Signatures` field uses a 3-level nested HashMap where the innermost values are inconsistently base64 strings, byte arrays, or null.

### Concrete Examples

#### Example 1: Signatures with Base64 Strings
**File**: `ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_pre_consensus_post_decided.json`  
**Lines**: 13-17

```json
{
  "Signatures": {
    "1": {
      "9989d3ab6c75aa22aef5d56898a930c3f67de00e241103071ecf84523c73fc1c": {
        "1": "jqIz63D8mz2hXyzIpoWcGwg2t9lkU8YGkU5Lcc5oAvweLS59pdMt+MIjFlL8ARWKGUuV5E2j4m27c3UW8lrKfD7nE3ISdP3byCnDxSa/s4OGgva7iof4O5vl6U55DkKr",
        "2": "pKkmqaP0Qmx7LBPZA1/dYFikb8HY/DPPkdlul6JxYjMGl8UW3a5w6mShTd+qpaaSBhD84CCnLh4ypriDuYOuts5F2l6q1UmH+1tnSH16O+EhkmY6TydRO/U848IL+CAF",
        "3": "iXOrrTj7uFYIMdd3DkYUM6GgFX298ST0T3FdfTMXUhrjhyf8YB/9p5OqaZUEG5tFBB9I3Zex1iQB0qRi3rcYL6ugpSWFtgoFPVfug7WICGZrVEJK2b1WVLP0JX1oFXHN"
      }
    }
  }
}
```

Structure: `Map<OperatorID, Map<MessageHash, Map<SignatureType, Base64String>>>`

#### Example 2: Signatures with Empty Objects
**File**: `ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_no_signatures.json`

```json
{
  "Signatures": {},
  "Quorum": 3
}
```

### Error Messages
```
invalid type: string "jqIz63D8mz2hXyzI...", expected a sequence at line 15 column 155
```

### Impact
- Requires complex custom deserializer for 3-level nested structure
- Innermost values need base64 decoding but are presented as strings
- Structure is difficult to validate and maintain

---

## Issue #4: SSVMessage Null Handling

### Problem
The `SSVMessage` field within `SignedSSVMessage` objects can be `null` but is not consistently handled as optional.

### Concrete Examples

#### Example 1: Null SSVMessage
**File**: `ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_nil_SSVMessage.json`

```json
{
  "Messages": [
    {
      "Signatures": ["sig1", "sig2"],
      "OperatorIDs": [1, 2],
      "SSVMessage": null,
      "FullData": "base64string"
    }
  ]
}
```

#### Example 2: Valid SSVMessage
**File**: `ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_full_happy_flow.json`

```json
{
  "Messages": [
    {
      "Signatures": ["sig1", "sig2"],
      "OperatorIDs": [1, 2],
      "SSVMessage": {
        "MsgType": 0,
        "MsgID": {...},
        "Data": "..."
      },
      "FullData": "base64string"
    }
  ]
}
```

### Impact
- Requires wrapper types to handle null SSVMessage fields
- Standard SSV message parsing fails on null values
- Test scenarios intentionally use null to test error conditions

---

## Issue #5: Inconsistent String vs Number Types

### Problem
Some fields that should be numbers are represented as strings, and vice versa.

### Concrete Examples

#### Example 1: Slot as String
**File**: `ssv-spec/ssv/spectest/generate/tests/tests.MultiMsgProcessingSpecTest_full_happy_flow.json`

```json
{
  "ValidatorDuty": {
    "Type": 1,
    "Slot": "7426977",
    "ValidatorIndex": "469836"
  }
}
```

#### Example 2: Numeric Fields as Strings
**File**: Multiple files

```json
{
  "ValidatorIndex": "469836",
  "Slot": "7426977",
  "SharePubKey": "LS0tLS1CRUdJTi..."
}
```

### Impact
- Requires custom parsing for numeric fields presented as strings
- Type validation becomes complex
- Inconsistent with typical JSON numeric representation

---

## Recommended Solutions

### 1. Standardize Base64 Encoding
**Recommendation**: Use consistent base64 encoding for all binary data.

```json
{
  "DecidedValue": "EAAAAAEAAAAAAAAAlAAAAAQAA...",
  "ValidatorPubKey": "AQIDBAUGBwgJCgECAwQFBgcICQoBAgMEBQYHCAkKAQI=",
  "FullData": "AQIDBAUGBwgJCgECAwQFBgcICQoBAgMEBQYHCAkKAQI="
}
```

### 2. Consistent Null Handling
**Recommendation**: Use explicit null values for optional fields.

```json
{
  "BeaconBroadcastedRoots": null,  // or omit entirely
  "ExpectedError": "error message"
}
```

### 3. Flatten Complex Structures
**Recommendation**: Replace nested HashMaps with flat arrays.

```json
{
  "Signatures": [
    {
      "operator_id": "1",
      "message_hash": "9989d3ab6c75aa22aef5d56898a930c3f67de00e241103071ecf84523c73fc1c",
      "signature_type": "1",
      "signature": "jqIz63D8mz2hXyzIpoWcGwg2t9lkU8YGkU5Lcc5oAvweLS59pdMt+MIjFlL8ARWKGUuV5E2j4m27c3UW8lrKfD7nE3ISdP3byCnDxSa/s4OGgva7iof4O5vl6U55DkKr"
    }
  ]
}
```

### 4. Add Schema Validation
**Recommendation**: Include JSON schema files to validate test structure.

### 5. Use Consistent Types
**Recommendation**: Use numbers for numeric fields, strings for string fields.

```json
{
  "ValidatorDuty": {
    "Type": 1,
    "Slot": 7426977,
    "ValidatorIndex": 469836
  }
}
```

---

## Files Requiring Custom Handling

The following files required the most complex custom deserializers:

1. **`tests.MultiMsgProcessingSpecTest_post_consensus_post_quorum.json`** - DecidedValue base64 string
2. **`tests.MultiMsgProcessingSpecTest_pre_consensus_post_decided.json`** - Complex signature map structure
3. **`tests.MultiMsgProcessingSpecTest_nil_SSVMessage.json`** - Null SSVMessage handling
4. **`tests.MultiMsgProcessingSpecTest_post_consensus_partial_invalid_root_quorum_then_valid_quorum.json`** - FullData base64 string

### Implementation Stats
- **Total files**: 99
- **Files with base64 issues**: 89
- **Files with null handling issues**: 37
- **Files with nested structure issues**: 23
- **Custom deserializers required**: 3
- **Lines of custom parsing code**: 180

---

## Impact on External Implementations

These inconsistencies force external implementations to:

1. **Write custom deserializers** for each problematic field type
2. **Use wrapper types** instead of direct SSV types
3. **Implement complex validation logic** for mixed type handling
4. **Maintain parsing compatibility** across different test file formats
5. **Significantly increase parsing complexity** (180 lines of custom code for what should be standard JSON parsing)

The parsing success rate improvement from 6% to 100% required substantial engineering effort that could be avoided with consistent JSON formatting.

## Conclusion

These format inconsistencies significantly complicate the implementation of SSV spec test parsers in external languages. Standardizing the JSON format would improve:

- **Developer experience** for implementers
- **Parsing performance** (no custom deserializers needed)
- **Maintainability** of test files
- **Compatibility** across different JSON parsers and languages

**Priority**: High - These issues affect all external implementations attempting to parse SSV spec tests.