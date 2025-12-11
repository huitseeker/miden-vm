// Allow unused assignments - required by miette::Diagnostic derive macro
#![allow(unused_assignments)]

//! # Error Architecture
//!
//! This module implements a two-tier error boundary pattern that separates "what went wrong"
//! (logical error semantics) from "where it went wrong" (diagnostic source context).
//!
//! ## Error Types
//!
//! - **[`OperationError`]**: Context-free errors from operations. Contains runtime data (clock
//!   cycles, values, addresses) but NO source locations. Returned by all operation implementations.
//!
//! - **[`ExecutionError`]**: User-facing errors with source spans and file references. Either wraps
//!   an `OperationError` with source context via the `OperationError` variant, or represents
//!   program-level errors (e.g., `CycleLimitExceeded`, `ProgramAlreadyExecuted`).
//!
//! ## Design Principles
//!
//! 1. **Operations return `OperationError`** - No error context threading through signatures. Each
//!    operation implementation is context-free and focuses purely on the error condition.
//!
//! 2. **Boundaries wrap with context** - Error context is added at boundaries where it's available
//!    (decoders, fast processor, basic block executors) using the `ErrorContext` struct and
//!    `OperationResultExt` trait.
//!
//! 3. **Errors propagate naturally** - No intermediate rewrapping. When a dyncall or call fails
//!    during callee execution, the error bubbles up with its original source context preserved,
//!    pointing to the actual failing instruction, not the call site.
//!
//! 4. **Subsystem errors appear in `OperationError` only** - Errors from chiplets ([`MemoryError`],
//!    [`AceError`]) are wrapped in `OperationError` at chiplet boundaries, then wrapped again in
//!    `ExecutionError` at operation boundaries. This creates a consistent error chain without
//!    ambiguity.
//!
//! ## Example Flow
//!
//! ```text
//! // 1. Operation (context-free)
//! fn op_u32add(&mut self) -> Result<(), OperationError> {
//!     if !is_valid {
//!         return Err(OperationError::NotU32Values { values, err_code });
//!     }
//!     Ok(())
//! }
//!
//! // 2. Boundary (adds context)
//! let ctx = ErrorContext::with_op(program, node_id, op_idx);
//! self.execute_op(op)
//!     .map_exec_err(&ctx, host, self.clk)?;
//! ```
//!
//! ## Error Context Feature Flag
//!
//! The `no_err_ctx` feature flag allows compile-time elimination of error context for
//! performance-critical builds. When enabled, error context operations become no-ops.

use alloc::{boxed::Box, sync::Arc, vec::Vec};

use miden_air::RowIndex;
use miden_core::{
    EventId, EventName, Felt, QuadFelt, Word,
    mast::{DecoratorId, MastForest, MastNodeId},
    stack::MIN_STACK_DEPTH,
    utils::to_hex,
};
use miden_debug_types::{SourceFile, SourceSpan};
use miden_utils_diagnostics::{Diagnostic, miette};
use winter_prover::ProverError;

use crate::{
    AssertError, BaseHost, DebugError, EventError, MemoryError, TraceError,
    host::advice::AdviceError,
};

// EXECUTION ERROR
// ================================================================================================

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum ExecutionError {
    #[error("exceeded the allowed number of max cycles {0}")]
    CycleLimitExceeded(u32),
    #[error("attempted to add event handler for '{event}' (already registered)")]
    DuplicateEventHandler { event: EventName },
    #[error("attempted to add event handler for '{event}' (reserved system event)")]
    ReservedEventNamespace { event: EventName },
    #[error("failed to execute the program for internal reason: {0}")]
    FailedToExecuteProgram(&'static str),
    #[error("stack should have at most {MIN_STACK_DEPTH} elements at the end of program execution, but had {} elements", MIN_STACK_DEPTH + .0)]
    OutputStackOverflow(usize),
    #[error("a program has already been executed in this process")]
    ProgramAlreadyExecuted,
    #[error("failed to initialize the program")]
    ProgramInitializationFailed,
    #[error("proof generation failed")]
    ProverError(#[source] ProverError),
    #[error("execution yielded unexpected precompiles")]
    UnexpectedPrecompiles,
    #[error("debug handler error at clock cycle {clk}: {err}")]
    DebugHandlerError {
        clk: RowIndex,
        #[source]
        err: DebugError,
    },
    #[error("trace handler error at clock cycle {clk} for trace ID {trace_id}: {err}")]
    TraceHandlerError {
        clk: RowIndex,
        trace_id: u32,
        #[source]
        err: TraceError,
    },
    #[error("operation error at clock cycle {clk}")]
    #[diagnostic()]
    OperationError {
        clk: RowIndex,
        #[label]
        label: SourceSpan,
        #[source_code]
        source_file: Option<Arc<SourceFile>>,
        #[source]
        err: Box<OperationError>,
    },
    #[error("operation error at clock cycle {clk} (source location unavailable)")]
    #[diagnostic(help(
        "this error occurred during execution, but source location information is not available. This typically happens when loading external MAST forests without debug information"
    ))]
    OperationErrorNoContext {
        clk: RowIndex,
        #[source]
        err: Box<OperationError>,
    },
}

impl AsRef<dyn Diagnostic> for ExecutionError {
    fn as_ref(&self) -> &(dyn Diagnostic + 'static) {
        self
    }
}

// OPERATION ERROR
// ================================================================================================

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum OperationError {
    #[error("advice provider error")]
    #[diagnostic(transparent)]
    AdviceError(
        #[from]
        #[source]
        #[diagnostic_source]
        AdviceError,
    ),
    #[error("failed to execute the program for internal reason: {0}")]
    FailedToExecuteProgram(&'static str),
    #[error("division by zero")]
    #[diagnostic()]
    DivideByZero,
    #[error("failed to execute the dynamic code block provided by the stack with root {hex}; the block could not be found",
      hex = .digest.to_hex()
    )]
    #[diagnostic()]
    DynamicNodeNotFound { digest: Word },
    #[error("error during processing of event {}", match event_name {
        Some(name) => format!("'{}' (ID: {})", name, event_id),
        None => format!("with ID: {}", event_id),
    })]
    #[diagnostic()]
    EventError {
        event_id: EventId,
        event_name: Option<EventName>,
        #[source]
        error: EventError,
    },
    #[error("assertion failed with error {}{}",
      match err_msg {
        Some(msg) => format!("message: {msg}"),
        None => format!("code: {err_code}"),
      },
      match err {
        Some(err) => format!(" (host error: {err})"),
        None => alloc::string::String::new(),
      }
    )]
    #[diagnostic()]
    FailedAssertion {
        err_code: Felt,
        err_msg: Option<Arc<str>>,
        #[source]
        err: Option<AssertError>,
    },
    #[error(
        "when returning from a call or dyncall, stack depth must be {MIN_STACK_DEPTH}, but was {depth}"
    )]
    #[diagnostic()]
    InvalidStackDepthOnReturn { depth: usize },
    #[error("attempted to calculate integer logarithm with zero argument")]
    #[diagnostic()]
    LogArgumentZero,
    #[error("malformed signature key: {key_type}")]
    #[diagnostic(help("the secret key associated with the provided public key is malformed"))]
    MalformedSignatureKey { key_type: &'static str },
    #[error("merkle path verification failed for value {value} at index {index} in the Merkle tree with root {root} (error {err})",
      value = to_hex(value.as_bytes()),
      root = to_hex(root.as_bytes()),
      err = match err_msg {
        Some(msg) => format!("message: {msg}"),
        None => format!("code: {err_code}"),
      }
    )]
    MerklePathVerificationFailed {
        value: Word,
        index: Felt,
        root: Word,
        err_code: Felt,
        err_msg: Option<Arc<str>>,
    },
    #[error("if statement expected a binary value on top of the stack, but got {value}")]
    #[diagnostic()]
    NotBinaryValueIf { value: Felt },
    #[error("operation expected a binary value, but got {value}")]
    #[diagnostic()]
    NotBinaryValueOp { value: Felt },
    #[error("loop condition must be a binary value, but got {value}")]
    #[diagnostic(help(
        "this could happen either when first entering the loop, or any subsequent iteration"
    ))]
    NotBinaryValueLoop { value: Felt },
    #[error("operation expected u32 values, but got values: {values:?} (error code: {err_code})")]
    NotU32Values { values: Vec<Felt>, err_code: Felt },
    #[error("Operand stack input is {input} but it is expected to fit in a u32")]
    #[diagnostic()]
    NotU32StackValue { input: u64 },
    #[error("smt node {node_hex} not found", node_hex = to_hex(node.as_bytes()))]
    SmtNodeNotFound { node: Word },
    #[error("expected pre-image length of node {node_hex} to be a multiple of 8 but was {preimage_len}",
      node_hex = to_hex(node.as_bytes()),
    )]
    SmtNodePreImageNotValid { node: Word, preimage_len: usize },
    #[error("stack overflow")]
    #[diagnostic()]
    StackOverflow,
    #[error("syscall failed: procedure with root {hex} was not found in the kernel",
      hex = to_hex(proc_root.as_bytes())
    )]
    SyscallTargetNotInKernel { proc_root: Word },
    #[error("failed to execute arithmetic circuit evaluation operation: {error}")]
    #[diagnostic()]
    AceChipError { error: AceError },
    #[error(transparent)]
    #[diagnostic(transparent)]
    MemoryError(#[from] MemoryError),
    #[error(
        "invalid crypto operation: Merkle path length {path_len} does not match expected depth {depth}"
    )]
    #[diagnostic()]
    InvalidCryptoInput { path_len: usize, depth: Felt },
    #[error("FRI domain segment value cannot exceed 3, but was {0}")]
    InvalidFriDomainSegment(u64),
    #[error("degree-respecting projection is inconsistent: expected {0} but was {1}")]
    InvalidFriLayerFolding(QuadFelt, QuadFelt),
    #[error("external node with mast root {0} resolved to an external node")]
    CircularExternalNode(Word),
    #[error("decorator id {decorator_id} does not exist in MAST forest")]
    DecoratorNotFoundInForest { decorator_id: DecoratorId },
    #[error("node id {node_id} does not exist in MAST forest")]
    MastNodeNotFoundInForest { node_id: MastNodeId },
    #[error("no MAST forest contains the procedure with root digest {root_digest}")]
    NoMastForestWithProcedure { root_digest: Word },
    #[error(
        "MAST forest in host indexed by procedure root {root_digest} doesn't contain that root"
    )]
    MalformedMastForestInHost { root_digest: Word },
    #[error("exceeded the allowed number of max cycles {max_cycles}")]
    CycleLimitExceeded { max_cycles: u32 },
    #[error("decorator execution error")]
    #[diagnostic(transparent)]
    DecoErr(
        #[source]
        #[diagnostic_source]
        Box<ExecutionError>,
    ),
}

// ACE ERROR
// ================================================================================================

#[derive(Debug, thiserror::Error)]
pub enum AceError {
    #[error("num of variables should be word aligned and non-zero but was {0}")]
    NumVarIsNotWordAlignedOrIsEmpty(u64),
    #[error("num of evaluation gates should be word aligned and non-zero but was {0}")]
    NumEvalIsNotWordAlignedOrIsEmpty(u64),
    #[error("circuit does not evaluate to zero")]
    CircuitNotEvaluateZero,
    #[error("failed to read from memory")]
    FailedMemoryRead,
    #[error("failed to decode instruction")]
    FailedDecodeInstruction,
    #[error("failed to read from the wiring bus")]
    FailedWireBusRead,
    #[error("num of wires must be less than 2^30 but was {0}")]
    TooManyWires(u64),
}

// ERROR CONTEXT
// ===============================================================================================

/// Lightweight error context handle for lazy source location resolution.
///
/// This struct stores only references and scalars needed to resolve error context later (in the
/// error path). This avoids the cost of MAST traversal and host lookups on the hot success path.
///
/// # Performance
///
/// - **Construction**: Nearly free - just stores pointers and scalars (no MAST walk, no host calls)
/// - **Resolution**: Only happens inside `.map_err()` closure when error actually occurs
///
/// # Feature Flags
///
/// When `no_err_ctx` is enabled, this struct collapses to zero-cost.
#[cfg(not(feature = "no_err_ctx"))]
pub struct ErrorContext<'a> {
    program: &'a MastForest,
    node_id: MastNodeId,
    op_idx: Option<usize>,
}

#[cfg(feature = "no_err_ctx")]
pub struct ErrorContext<'a> {
    _phantom: core::marker::PhantomData<&'a ()>,
}

#[cfg(not(feature = "no_err_ctx"))]
impl<'a> ErrorContext<'a> {
    /// Creates a new error context for a MAST node without a specific operation index.
    pub fn new(program: &'a MastForest, node_id: MastNodeId) -> Self {
        Self { program, node_id, op_idx: None }
    }

    /// Creates a new error context for a specific operation within a MAST node.
    pub fn with_op(program: &'a MastForest, node_id: MastNodeId, op_idx: usize) -> Self {
        Self { program, node_id, op_idx: Some(op_idx) }
    }

    /// Resolves the error context to a source span and file, if available.
    ///
    /// This is where the actual MAST traversal and host lookup happens.
    pub fn resolve(&self, host: &impl BaseHost) -> Option<(SourceSpan, Option<Arc<SourceFile>>)> {
        // Check if node_id is valid before calling get_assembly_op (which panics on invalid index)
        let node_idx = u32::from(self.node_id) as usize;
        if node_idx >= self.program.nodes().len() {
            return None;
        }

        self.program
            .get_assembly_op(self.node_id, self.op_idx)
            .and_then(|assembly_op| assembly_op.location())
            .map(|location| host.get_label_and_source_file(location))
    }

    /// Converts an `OperationError` into an `ExecutionError` with source context.
    pub fn into_exec_err(
        &self,
        host: &impl BaseHost,
        err: OperationError,
        clk: RowIndex,
    ) -> ExecutionError {
        match self.resolve(host) {
            Some((label, source_file)) => ExecutionError::OperationError {
                clk,
                label,
                source_file,
                err: Box::new(err),
            },
            None => ExecutionError::OperationErrorNoContext { clk, err: Box::new(err) },
        }
    }

    /// Enriches an existing `ExecutionError` with source context if it doesn't have any.
    ///
    /// This is used when an error propagates from a nested call and we want to preserve
    /// the original error's context, or add context if it was missing.
    pub fn enrich_exec_err(&self, host: &impl BaseHost, err: ExecutionError) -> ExecutionError {
        // If the error already has context, return it unchanged
        match err {
            ExecutionError::OperationError { .. } => err,
            ExecutionError::OperationErrorNoContext { clk, err: op_err } => {
                // Try to add context if we can resolve it
                match self.resolve(host) {
                    Some((label, source_file)) => {
                        ExecutionError::OperationError { clk, label, source_file, err: op_err }
                    },
                    None => ExecutionError::OperationErrorNoContext { clk, err: op_err },
                }
            },
            // For all other error types, return unchanged
            _ => err,
        }
    }
}

#[cfg(feature = "no_err_ctx")]
impl<'a> ErrorContext<'a> {
    /// Creates a new error context (no-op when `no_err_ctx` is enabled).
    pub fn new(_program: &'a MastForest, _node_id: MastNodeId) -> Self {
        Self { _phantom: core::marker::PhantomData }
    }

    /// Creates a new error context with operation index (no-op when `no_err_ctx` is enabled).
    pub fn with_op(_program: &'a MastForest, _node_id: MastNodeId, _op_idx: usize) -> Self {
        Self { _phantom: core::marker::PhantomData }
    }

    /// Resolves the error context (returns None when `no_err_ctx` is enabled).
    pub fn resolve(&self, _host: &impl BaseHost) -> Option<(SourceSpan, Option<Arc<SourceFile>>)> {
        None
    }

    /// Converts an `OperationError` into an `ExecutionError` without context.
    pub fn into_exec_err(
        &self,
        _host: &impl BaseHost,
        err: OperationError,
        clk: RowIndex,
    ) -> ExecutionError {
        ExecutionError::OperationErrorNoContext { clk, err: Box::new(err) }
    }

    /// Enriches an existing `ExecutionError` (no-op when `no_err_ctx` is enabled).
    pub fn enrich_exec_err(&self, _host: &impl BaseHost, err: ExecutionError) -> ExecutionError {
        err
    }
}

// OPERATION RESULT EXTENSION
// ===============================================================================================

/// Extension trait for `Result<T, OperationError>` to simplify conversion to `ExecutionError`.
pub trait OperationResultExt<T> {
    /// Converts `Result<T, OperationError>` to `Result<T, ExecutionError>` without context.
    fn map_exec_err_no_ctx(self, clk: RowIndex) -> Result<T, ExecutionError>;

    /// Converts `Result<T, OperationError>` to `Result<T, ExecutionError>` with context.
    fn map_exec_err(
        self,
        err_ctx: &ErrorContext,
        host: &impl BaseHost,
        clk: RowIndex,
    ) -> Result<T, ExecutionError>;
}

impl<T> OperationResultExt<T> for Result<T, OperationError> {
    fn map_exec_err_no_ctx(self, clk: RowIndex) -> Result<T, ExecutionError> {
        self.map_err(|err| ExecutionError::OperationErrorNoContext { clk, err: Box::new(err) })
    }

    fn map_exec_err(
        self,
        err_ctx: &ErrorContext,
        host: &impl BaseHost,
        clk: RowIndex,
    ) -> Result<T, ExecutionError> {
        self.map_err(|err| err_ctx.into_exec_err(host, err, clk))
    }
}

impl OperationResultExt<()> for OperationError {
    fn map_exec_err_no_ctx(self, clk: RowIndex) -> Result<(), ExecutionError> {
        Err(ExecutionError::OperationErrorNoContext { clk, err: Box::new(self) })
    }

    fn map_exec_err(
        self,
        err_ctx: &ErrorContext,
        host: &impl BaseHost,
        clk: RowIndex,
    ) -> Result<(), ExecutionError> {
        Err(err_ctx.into_exec_err(host, self, clk))
    }
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod error_assertions {
    use super::*;

    /// Asserts at compile time that the passed error has Send + Sync + 'static bounds.
    fn _assert_error_is_send_sync_static<E: core::error::Error + Send + Sync + 'static>(_: E) {}

    fn _assert_execution_error_bounds(err: ExecutionError) {
        _assert_error_is_send_sync_static(err);
    }

    fn _assert_operation_error_bounds(err: OperationError) {
        _assert_error_is_send_sync_static(err);
    }
}
