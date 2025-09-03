#!/bin/bash

# Solana SBPF zkVM Build and Run Script
# This script builds the Solana BPF program and executes it in RISC Zero zkVM

set -e

echo "========================================="
echo "Solana SBPF Runtime in RISC Zero zkVM"
echo "========================================="
echo ""

# Check if cargo-build-sbpf is installed
if ! command -v cargo-build-sbf &> /dev/null; then
    echo "Error: cargo-build-sbf is not installed."
    echo "Please install Solana CLI tools:"
    echo "  sh -c \"\$(curl -sSfL https://release.solana.com/stable/install)\""
    exit 1
fi

# Step 1: Build the Solana BPF program
echo "[1/3] Building Solana BPF program..."
echo "-------------------------------------"
cd minimal-sol
cargo-build-sbf
cd ..
echo "✓ Solana BPF program built successfully"
echo ""

# Step 2: Build the RISC Zero project
echo "[2/3] Building RISC Zero zkVM project..."
echo "----------------------------------------"
cargo build --release
echo "✓ RISC Zero project built successfully"
echo ""

# Step 3: Run the zkVM prover in development mode
echo "[3/3] Running zkVM prover (dev mode)..."
echo "---------------------------------------"
RISC0_DEV_MODE=1 cargo run --release

echo ""
echo "========================================="
echo "Execution completed successfully!"
echo "========================================="
