#!/bin/bash
set -e

# Integration Suite for Wardex
# Usage: ./tests/integration_suite.sh [path_to_binary]
# If path_to_binary is not provided, it assumes 'wardex' is in PATH or built in target/debug

BINARY=${1:-"../target/debug/wardex"}
# Resolve absolute path
BINARY=$(realpath "$BINARY")
TEST_DIR=$(mktemp -d)
CONFIG_FILE="$TEST_DIR/config.yaml"

echo "🧪 Running Wardex Integration Suite"
echo "📂 Test Workspace: $TEST_DIR"
echo "🔨 Binary: $BINARY"

# 1. Setup Environment
mkdir -p "$TEST_DIR/workspace/0_Inbox"
mkdir -p "$TEST_DIR/workspace/1_Projects"
mkdir -p "$TEST_DIR/workspace/2_Areas"
mkdir -p "$TEST_DIR/workspace/3_Resources"
mkdir -p "$TEST_DIR/workspace/4_Archives"

# Mock Config
cat > "$CONFIG_FILE" <<EOF
paths:
  workspace: "$TEST_DIR/workspace"
  inbox: "$TEST_DIR/workspace/0_Inbox"
  projects: "$TEST_DIR/workspace/1_Projects"
  areas: "$TEST_DIR/workspace/2_Areas"
  resources: "$TEST_DIR/workspace/3_Resources"
  archives: "$TEST_DIR/workspace/4_Archives"
rules:
  clean: []
organize:
  ctf_dir: CTFs
ctf:
  default_categories: [pwn, crypto, web]
EOF

CMD="$BINARY --config $CONFIG_FILE"

# 2. Test Init (Scaffolding)
echo "------------------------------------------------"
echo "Testing: Project Scaffolding"
$CMD init --type rust --name test-rust-proj
$CMD init --type python --name test-py-proj

if [ -d "$TEST_DIR/workspace/1_Projects/test-rust-proj" ]; then echo "✅ Rust Project Created"; else echo "❌ Rust Project Failed"; exit 1; fi
if [ -f "$TEST_DIR/workspace/1_Projects/test-py-proj/main.py" ]; then echo "✅ Python Project Created"; else echo "❌ Python Project Failed"; exit 1; fi

# 3. Test Stats 
echo "------------------------------------------------"
echo "Testing: Stats"
$CMD stats | grep "Projects"
echo "✅ Stats command ran successfully"

# 3.5 Test Robust Config (Minimal)
echo "------------------------------------------------"
echo "Testing: Robust Config (Minimal)"
MINIMAL_CONFIG="$TEST_DIR/minimal_config.yaml"
cat > "$MINIMAL_CONFIG" <<EOF
paths:
  workspace: "$TEST_DIR/workspace"
EOF
$BINARY --config "$MINIMAL_CONFIG" status
echo "✅ Minimal config Status command passed"

# 4. Test CTF Workflow
echo "------------------------------------------------"
echo "Testing: CTF Workflow"
# Init Event
$CMD ctf init "DefCon_Qualifier" --date "2024-05-01"
EVENT_DIR="$TEST_DIR/workspace/1_Projects/CTFs/2024_DefCon_Qualifier"
if [ -d "$EVENT_DIR/pwn" ]; then echo "✅ CTF Event Initialized"; else echo "❌ CTF Init Failed"; exit 1; fi

# Add Challenge
cd "$EVENT_DIR"
$CMD ctf add "pwn/buffer_overflow"
if [ -f "$EVENT_DIR/pwn/buffer_overflow/solve.py" ]; then echo "✅ Challenge Added"; else echo "❌ Challenge Add Failed"; exit 1; fi

# Generate Writeup
echo "Flag is flag{test_flag}" > "$EVENT_DIR/pwn/buffer_overflow/notes.md"
$CMD ctf writeup
if [ -f "$EVENT_DIR/Writeup.md" ]; then echo "✅ Writeup Generated"; else echo "❌ Writeup Generation Failed"; exit 1; fi

# Archive Event
$CMD ctf archive "DefCon_Qualifier"
ARCHIVE_DIR="$TEST_DIR/workspace/4_Archives/CTFs/2024/2024_DefCon_Qualifier"
if [ -d "$ARCHIVE_DIR" ]; then echo "✅ Event Archived"; else echo "❌ Event Archive Failed"; exit 1; fi

# 5. Test Search (Find & Grep)
echo "------------------------------------------------"
echo "Testing: Enhanced Search"
# Create dummy resource
mkdir -p "$TEST_DIR/workspace/3_Resources/Cheatsheets"
echo "some interesting content with flag{hidden_pattern}" > "$TEST_DIR/workspace/3_Resources/Cheatsheets/pwn.txt"

# Grep
echo "Running Grep..."
$CMD grep "flag" | grep "pwn.txt"
echo "✅ Grep Found Content"

# Find
echo "Running Find..."
$CMD find "test-rust" | grep "test-rust-proj"
echo "✅ Fuzzy Find Found Project"

# 6. Test Dev Tools - REMOVED
# Dev tools have been deprioritized.

echo "------------------------------------------------"
echo "🎉 All Integration Tests Passed!"
rm -rf "$TEST_DIR"
