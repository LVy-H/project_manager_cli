#!/bin/bash
set -e

BIN="/app/target/release/folder_manager"

echo "--- RUNNING INTEGRATION TESTS ---"

echo "[1] Testing Status (Clean)..."
$BIN status

echo "[2] Testing Status (Dirty)..."
cd 1_Projects/MyRepo
echo "dirty" >> README.md
cd ../..
$BIN status

echo "[3] Testing Clean (Dry Run)..."
touch 0_Inbox/ctf_challenge.zip
$BIN clean --dry-run

echo "[4] Testing Clean (Real)..."
$BIN clean
if [ -f "1_Projects/CTFs/ctf_challenge.zip" ]; then
    echo "SUCCESS: File moved correctly."
else
    echo "FAILURE: File not moved."
    exit 1
fi

echo "[5] Testing Undo..."
$BIN undo
if [ -f "0_Inbox/ctf_challenge.zip" ]; then
    echo "SUCCESS: Undo worked."
else
    echo "FAILURE: Undo failed."
    exit 1
fi

echo "ALL TESTS PASSED"
