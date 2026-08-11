//! Fuzz target for ExecutionProof deserialization.
//!
//! Run with: cargo +nightly fuzz run execution_proof_deserialize --fuzz-dir tools/miden-core-fuzz

#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use miden_core::{deferred::PrecompileRegistry, proof::ExecutionProof};

fuzz_target!(|data: &[u8]| {
    // An empty registry intentionally exercises framework-only decoding.
    let registry = Arc::new(PrecompileRegistry::new());
    let _ = ExecutionProof::read_from_bytes(data, registry);
});
