#!/usr/bin/env python3
"""
Temporary feedback loop for: Fix all remaining QBFT controller test failures
This script will iterate with Claude until all tests pass.
"""

import subprocess
import json
import sys
import time
import os
import re
from datetime import datetime
from pathlib import Path

class FeedbackLoop:
    def __init__(self):
        self.goal = "Fix all remaining QBFT controller test failures"
        self.max_iterations = 8
        self.iteration = 0
        self.start_time = datetime.now()
        self.log_file = f"feedback_loop_{datetime.now().strftime('%Y%m%d_%H%M%S')}.log"
        self.summary_file = f"feedback_loop_summary_{datetime.now().strftime('%Y%m%d_%H%M%S')}.md"
        self.iterations_summary = []
        self.last_test_output = ""
        self.last_error = ""
        self.current_test_result = {}
        self.current_claude_analysis = ""
        self.current_changes_made = []
        
    def test_condition(self):
        """Test if all QBFT controller tests pass."""
        try:
            # Run the specific controller test
            result = subprocess.run(
                ["cargo", "test", "test_qbft_controller", "--", "--nocapture"], 
                capture_output=True, 
                text=True,
                cwd="/home/dsfreakdude/code/sigp/anchor/anchor/spec_tests",
                timeout=300  # 5 minute timeout for test run
            )
            
            self.last_test_output = result.stdout + result.stderr
            self.last_error = result.stderr
            
            if result.returncode != 0:
                # Parse test failures
                failures = self.parse_test_failures(result.stdout + result.stderr)
                
                self.current_test_result = {
                    "passed": False,
                    "reason": "QBFT controller tests failed",
                    "details": f"{len(failures)} failing tests: {', '.join(failures[:3])}{'...' if len(failures) > 3 else ''}"
                }
                return False
            
            self.current_test_result = {
                "passed": True, 
                "reason": "All QBFT controller tests passing", 
                "details": "53/53 tests successful"
            }
            return True
            
        except subprocess.TimeoutExpired:
            self.current_test_result = {
                "passed": False,
                "reason": "Test timeout",
                "details": "Tests took longer than 5 minutes"
            }
            return False
        except Exception as e:
            self.log(f"Test execution failed: {e}")
            self.current_test_result = {
                "passed": False,
                "reason": "Exception during test execution",
                "details": str(e)
            }
            return False
    
    def parse_test_failures(self, output):
        """Extract failing test names from test output."""
        failures = []
        lines = output.split('\n')
        
        for line in lines:
            if '❌ Test ' in line:
                # Extract test name from "❌ Test 'test_name' failed!"
                match = re.search(r"❌ Test '([^']+)' failed!", line)
                if match:
                    failures.append(match.group(1))
            elif '✗ Controller test ' in line:
                # Extract test name from "✗ Controller test 'test_name' failed:"
                match = re.search(r"✗ Controller test '([^']+)' failed:", line)
                if match:
                    failures.append(match.group(1))
        
        return failures
    
    def get_context(self):
        """Gather context for Claude about the current state."""
        # Parse specific failure patterns
        failure_analysis = self.analyze_failures()
        
        context = {
            "goal": self.goal,
            "iteration": self.iteration,
            "test_output": self.last_test_output[-3000:],  # Last 3000 chars
            "failure_analysis": failure_analysis,
            "relevant_files": self.get_relevant_files(),
            "error_details": self.last_error[-1000:] if self.last_error else ""
        }
        
        return context
    
    def analyze_failures(self):
        """Analyze test failures to identify patterns."""
        analysis = {
            "hash_mismatches": [],
            "missing_controller_roots": [],
            "wrong_decided_counts": [],
            "other_failures": []
        }
        
        lines = self.last_test_output.split('\n')
        current_test = None
        
        for line in lines:
            # Track current test
            if 'Controller test ' in line and (' passed' in line or ' failed' in line):
                match = re.search(r"Controller test '([^']+)'", line)
                if match:
                    current_test = match.group(1)
            
            # Categorize failures
            if current_test and 'Controller root mismatch' in line:
                # Extract expected and actual hashes
                expected_match = re.search(r'expected ([a-f0-9]{64})', line)
                actual_match = re.search(r'got ([a-f0-9]{64})', line)
                analysis["hash_mismatches"].append({
                    "test": current_test,
                    "expected": expected_match.group(1) if expected_match else "unknown",
                    "actual": actual_match.group(1) if actual_match else "unknown"
                })
            elif current_test and 'but got None' in line:
                analysis["missing_controller_roots"].append(current_test)
            elif current_test and 'Decided count mismatch' in line:
                analysis["wrong_decided_counts"].append(current_test)
            elif current_test and ('failed:' in line or '✗' in line):
                analysis["other_failures"].append(current_test)
        
        return analysis
    
    def get_relevant_files(self):
        """Identify files Claude should focus on."""
        return [
            "src/qbft/adapter/simple_controller_test.rs",
            "src/qbft/adapter/types.rs", 
            "src/qbft/adapter/unified.rs",
            "src/qbft/controller_test.rs"
        ]
    
    def call_claude(self, context):
        """Call Claude with the current context."""
        failure_summary = self.create_failure_summary(context["failure_analysis"])
        
        prompt = f'''Goal: {self.goal}

Current iteration: {self.iteration}/{self.max_iterations}

QBFT controller tests are failing. Here's the analysis:

FAILURE SUMMARY:
{failure_summary}

TEST OUTPUT (last 3000 chars):
{context["test_output"]}

FAILURE ANALYSIS:
{json.dumps(context["failure_analysis"], indent=2)}

Please analyze the failing tests and make targeted changes to fix them. Focus on:

1. Hash mismatches - these indicate JSON serialization field ordering issues
2. Missing controller roots - tests expect a hash but get None  
3. Wrong decided counts - incorrect decision state handling

Key files to focus on: {', '.join(context["relevant_files"])}

IMPORTANT:
- Make minimal, targeted changes
- Focus on systematic fixes that address multiple similar failures
- Consider that we already fixed "decide current instance" successfully - use that as a reference
- Start your response with "ANALYSIS:" followed by a brief explanation of the pattern you see
- Then use "CHANGES:" followed by specific changes you'll make
'''
        
        # Store for summary
        self.current_claude_analysis = ""
        self.current_changes_made = []
        
        # Call Claude CLI
        cmd = ["claude", "--json"]
        process = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        
        stdout, stderr = process.communicate(input=prompt)
        
        if process.returncode != 0:
            self.log(f"Claude call failed: {stderr}")
            return False
            
        # Parse Claude's response and extract summary info
        try:
            full_response = []
            for line in stdout.strip().split('\n'):
                if line.strip():
                    data = json.loads(line)
                    if data.get("type") == "text":
                        text = data.get('text', '')
                        full_response.append(text)
                        self.log(f"Claude: {text[:200]}...")
            
            # Extract analysis and changes from response
            response_text = '\n'.join(full_response)
            if "ANALYSIS:" in response_text:
                analysis_start = response_text.find("ANALYSIS:") + 9
                analysis_end = response_text.find("CHANGES:") if "CHANGES:" in response_text else len(response_text)
                self.current_claude_analysis = response_text[analysis_start:analysis_end].strip()[:300]
            
            if "CHANGES:" in response_text:
                changes_start = response_text.find("CHANGES:") + 8
                changes_text = response_text[changes_start:].strip()[:500]
                self.current_changes_made = [line.strip() for line in changes_text.split('\n')[:5] if line.strip()]
                
        except json.JSONDecodeError as e:
            self.log(f"Failed to parse Claude response: {e}")
            
        return True
    
    def create_failure_summary(self, analysis):
        """Create a concise summary of current failures."""
        summary = []
        
        if analysis["hash_mismatches"]:
            summary.append(f"Hash Mismatches: {len(analysis['hash_mismatches'])} tests")
            for failure in analysis["hash_mismatches"][:3]:
                summary.append(f"  - {failure['test']}: expected {failure['expected'][:16]}..., got {failure['actual'][:16]}...")
        
        if analysis["missing_controller_roots"]:
            summary.append(f"Missing Controller Roots: {len(analysis['missing_controller_roots'])} tests")
            summary.append(f"  - {', '.join(analysis['missing_controller_roots'][:3])}")
        
        if analysis["wrong_decided_counts"]:
            summary.append(f"Wrong Decided Counts: {len(analysis['wrong_decided_counts'])} tests")
            summary.append(f"  - {', '.join(analysis['wrong_decided_counts'][:3])}")
        
        if analysis["other_failures"]:
            summary.append(f"Other Failures: {len(analysis['other_failures'])} tests")
        
        return '\n'.join(summary)
    
    def log(self, message):
        """Log to file and console."""
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        log_entry = f"[{timestamp}] {message}"
        print(log_entry)
        with open(self.log_file, "a") as f:
            f.write(log_entry + "\n")
    
    def save_iteration_summary(self):
        """Save summary of current iteration."""
        summary = {
            "iteration": self.iteration,
            "test_result": self.current_test_result,
            "claude_analysis": self.current_claude_analysis,
            "changes_made": self.current_changes_made
        }
        self.iterations_summary.append(summary)
    
    def write_final_summary(self, success):
        """Write concise summary of all iterations."""
        with open(self.summary_file, "w") as f:
            f.write(f"# Feedback Loop Summary\n\n")
            f.write(f"**Goal**: {self.goal}\n")
            f.write(f"**Result**: {'✓ SUCCESS' if success else '✗ FAILED'}\n")
            f.write(f"**Total Iterations**: {self.iteration}\n")
            f.write(f"**Duration**: {(datetime.now() - self.start_time).total_seconds():.1f}s\n\n")
            
            f.write("## Iteration Details\n\n")
            
            for summary in self.iterations_summary:
                f.write(f"### Iteration {summary['iteration']}\n")
                f.write(f"**What Failed**: {summary['test_result']['reason']}")
                if summary['test_result']['details']:
                    f.write(f" - {summary['test_result']['details'][:150]}")
                f.write("\n")
                
                if summary['claude_analysis']:
                    f.write(f"**Claude's Analysis**: {summary['claude_analysis']}\n")
                
                if summary['changes_made']:
                    f.write(f"**Changes Made**: \n")
                    for change in summary['changes_made'][:5]:
                        if change.strip():
                            f.write(f"- {change.strip()}\n")
                f.write("\n")
            
            if success:
                f.write("## Success\n")
                f.write("All QBFT controller tests are now passing!\n")
            
        self.log(f"Summary written to: {self.summary_file}")
    
    def run(self):
        """Main feedback loop."""
        self.log(f"Starting feedback loop for: {self.goal}")
        
        while self.iteration < self.max_iterations:
            self.iteration += 1
            self.log(f"\n--- Iteration {self.iteration} ---")
            
            # Test current state
            self.log("Testing QBFT controller tests...")
            if self.test_condition():
                self.log("✓ Success! All QBFT controller tests are now passing.")
                self.current_test_result = {"passed": True, "reason": "All tests passing", "details": "53/53 tests successful"}
                self.save_iteration_summary()
                self.write_final_summary(True)
                return True
            
            # Get context and call Claude
            self.log("Tests still failing. Gathering context...")
            context = self.get_context()
            
            self.log("Calling Claude for assistance...")
            if not self.call_claude(context):
                self.log("Failed to get Claude's help. Retrying...")
                self.current_claude_analysis = "Failed to get Claude response"
                self.current_changes_made = ["No changes - Claude call failed"]
            
            # Save iteration summary
            self.save_iteration_summary()
            
            # Wait for changes to take effect
            self.log("Waiting for changes to take effect...")
            time.sleep(3)
        
        self.log(f"\n✗ Max iterations ({self.max_iterations}) reached without success.")
        self.write_final_summary(False)
        return False
    
    def cleanup(self):
        """Clean up temporary files if needed."""
        pass

if __name__ == "__main__":
    loop = FeedbackLoop()
    try:
        success = loop.run()
        sys.exit(0 if success else 1)
    except KeyboardInterrupt:
        loop.log("\nInterrupted by user")
        sys.exit(130)
    finally:
        loop.cleanup()