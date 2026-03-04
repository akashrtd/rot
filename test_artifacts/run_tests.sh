#!/bin/bash
# Comprehensive E2E Test Script for rot

ROT_BIN="./target/release/rot"
TEST_DIR="test_artifacts"
LOG_FILE="$TEST_DIR/test_log.txt"

# Initialize log
echo "=== ROT E2E Test Suite ===" > $LOG_FILE
echo "Started: $(date)" >> $LOG_FILE
echo "" >> $LOG_FILE

# Test counter
TEST_NUM=0
PASS=0
FAIL=0

run_test() {
    local test_name="$1"
    local command="$2"
    local expected="$3"
    
    TEST_NUM=$((TEST_NUM + 1))
    echo "Test $TEST_NUM: $test_name"
    echo "Test $TEST_NUM: $test_name" >> $LOG_FILE
    
    OUTPUT_FILE="$TEST_DIR/test_${TEST_NUM}_output.txt"
    
    # Run command with timeout
    if command -v gtimeout >/dev/null 2>&1; then
        TIMEOUT_CMD="gtimeout 60"
    else
        TIMEOUT_CMD=""
    fi
    
    if [ -n "$TIMEOUT_CMD" ]; then
        $TIMEOUT_CMD $command > "$OUTPUT_FILE" 2>&1
        EXIT_CODE=$?
    else
        $command > "$OUTPUT_FILE" 2>&1
        EXIT_CODE=$?
    fi
    
    # Check result
    if [ $EXIT_CODE -eq 0 ]; then
        if [ -n "$expected" ]; then
            if grep -q "$expected" "$OUTPUT_FILE"; then
                echo "  ✓ PASS" | tee -a $LOG_FILE
                PASS=$((PASS + 1))
            else
                echo "  ✗ FAIL - Expected pattern not found: $expected" | tee -a $LOG_FILE
                FAIL=$((FAIL + 1))
            fi
        else
            echo "  ✓ PASS" | tee -a $LOG_FILE
            PASS=$((PASS + 1))
        fi
    else
        echo "  ✗ FAIL - Exit code: $EXIT_CODE" | tee -a $LOG_FILE
        FAIL=$((FAIL + 1))
    fi
    
    echo "  Output saved to: $OUTPUT_FILE" >> $LOG_FILE
    echo "" >> $LOG_FILE
}

echo "Running tests..."
echo ""

# Test A: Basic Chat
run_test "Basic Chat" \
    "$ROT_BIN exec \"What is 5 + 7? Answer with just the number.\"" \
    "12"

# Test B.1: Bash Tool
run_test "Bash Tool - List Files" \
    "$ROT_BIN exec \"Use bash to list all .toml files\"" \
    ".toml"

# Test B.2: Read Tool
run_test "Read Tool - Cargo.toml" \
    "$ROT_BIN exec \"Read Cargo.toml and list the workspace members\"" \
    "rot-"

# Test B.3: Glob Tool
run_test "Glob Tool - Find Rust Files" \
    "$ROT_BIN exec \"Use glob to find all main.rs files\"" \
    "main.rs"

# Test C: RLM Mode (skip if no context file)
# Note: RLM requires --context flag which needs a file
# This is a limitation of the exec command for RLM testing

# Test F: Slash Commands (these need interactive mode, so we test what we can)
run_test "Help Command" \
    "$ROT_BIN --help" \
    "Recursive Operations Tool"

run_test "Providers List" \
    "$ROT_BIN providers" \
    "Available providers"

run_test "Models List" \
    "$ROT_BIN models" \
    "glm-5"

run_test "Tools List" \
    "$ROT_BIN tools" \
    "Tool"

# Summary
echo ""
echo "=== Test Summary ===" | tee -a $LOG_FILE
echo "Total Tests: $TEST_NUM" | tee -a $LOG_FILE
echo "Passed: $PASS" | tee -a $LOG_FILE
echo "Failed: $FAIL" | tee -a $LOG_FILE
echo "Completed: $(date)" | tee -a $LOG_FILE
