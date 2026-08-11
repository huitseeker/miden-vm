#![cfg_attr(not(feature = "std"), no_std)]
#![doc = include_str!("../README.md")]

extern crate alloc;

// EXPORTS
// ================================================================================================

pub use miden_assembly::{
    self as assembly, Assembler,
    ast::{Module, ModuleKind},
    diagnostics,
};
pub use miden_core::{
    program::ExecutionClaim,
    proof::{
        ExecutionProof, ExecutionProofError, ExecutionProofTransportError, HashFunction,
        PrecompileProof, StarkProof, VmProof,
    },
};
pub use miden_processor::{
    BaseHost, DefaultHost, ExecutionError, ExecutionOptions, ExecutionOutput, ExecutionWitness,
    FastProcessor, FutureMaybeSend, Host, KernelDescriptor, PrecompileWitness, Program,
    ProgramExecutor, ProgramInfo, StackInputs, SyncHost, VmWitness, ZERO, advice, crypto, field,
    operation::Operation, serde, trace, trace::VmTrace, utils,
};
pub use miden_prover::{InputError, Prover, ProverError, StackOutputs, Word, prove_sync};
pub use miden_verifier::{VerificationError, VerificationOutcome, Verifier};

/// Decodes an execution proof using the standard bundled precompile registry and fixed
/// deferred-state element ceiling.
///
/// Decoding establishes transport syntax, canonical representation, and deferred witness
/// hydration; it does not establish proof validity. Call [`Verifier::verify`] on the decoded value.
///
/// Use [`ExecutionProof::read_from_bytes`] directly when decoding against a custom registry.
pub fn read_execution_proof_from_bytes(
    bytes: &[u8],
) -> Result<ExecutionProof, ExecutionProofTransportError> {
    ExecutionProof::read_from_bytes(bytes, alloc::sync::Arc::new(miden_precompiles::registry()))
}

// (private) exports
// ================================================================================================

#[cfg(feature = "internal")]
pub mod internal;
