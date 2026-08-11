# Miden prover

This crate proves post-execution witnesses produced by the
[Miden processor](../processor/) using [Plonky3](https://github.com/0xMiden/Plonky3). The
synchronous `Prover` does not execute programs: it consumes `ExecutionWitness` values and borrows
`PrecompileWitness` values.

## Usage

`Prover` is synchronous and consumes post-execution witnesses:

- `Prover::prove(ExecutionWitness)` proves only the VM portion. It returns a `Complete` proof
  without a precompile artifact when the VM authenticated no precompile work, or `Deferred` with
  the singleton precompile witness retained for later proving.
- `Prover::prove_full(ExecutionWitness)` proves the VM and any precompile witness immediately in
  memory and returns a complete proof.
- `Prover::prove_precompile(&PrecompileWitness)` proves one singleton or merged witness without
  cloning its hydrated DAG, including a witness retained by `Deferred`.

Use `Prover::with_hash_fn` to select the proof hash function.

Tracing must be selected before execution starts. Ordinary `FastProcessor::execute*` calls use a
no-op tracer and return `ExecutionOutput`, which cannot be promoted to `ExecutionWitness` after the
run because it contains no replay data. Call `FastProcessor::execute_for_proving*` when the result
will be passed to `Prover`.

### Deferred precompile workflow

```rust,ignore
use miden_prover::{ExecutionProof, Prover};
use miden_verifier::Verifier;

// `witness` is an ExecutionWitness produced by FastProcessor.
let claim = witness.claim();
let prover = Prover::new();

let deferred = prover.prove(witness)?;
assert!(matches!(&deferred, ExecutionProof::Deferred { .. }));
assert!(!deferred.is_complete());

let precompile_proof = prover.prove_precompile(
    deferred
        .precompile_witness()
        .expect("a deferred proof retains its precompile witness"),
)?;
let complete = deferred.complete(precompile_proof)?;

let outcome = Verifier::new().verify(&claim, &complete)?;
assert!(outcome.is_complete());
assert_eq!(outcome.outstanding_precompile_root(), None);
```

For merged proving, `PrecompileWitness::merge` accepts only singleton witnesses. Given
`[one, one, two]`, it preserves that order and duplicate; attempting to merge the resulting merged
witness again is rejected. The input list is capped at `MAX_PRECOMPILE_ROOTS`, and the complete
merged state is capped at `MAX_DEFERRED_ELEMENTS`. Proving the merged witness once produces
one shared `PrecompileProof` that can be cloned to complete compatible deferred proofs for `one` or
`two`. This is proof-artifact reuse, not a protocol batch or settlement envelope.

`PrecompileWitness` may contain private execution data and a large hydrated DAG. Treat it as
sensitive prover input, transport it only to trusted workers, and borrow it for proving. Clone it
only when an ownership-requiring transport needs a separate value. This crate does not currently
expose a delegated VM-worker proving or transport interface.

### Transport and verification

Encoding a witness or execution proof preserves its representation; it does not establish validity.
Malformed cross-artifact `ExecutionProof` values can therefore serialize and decode, but full
verification rejects inconsistent structure or invalid STARKs.

Ordinary façade callers can decode with the standard bundled registry via
`miden_vm::read_execution_proof_from_bytes(bytes)`. Custom-precompile callers use
`ExecutionProof::read_from_bytes(bytes, registry)` directly.

The deferred-wire canonical decode-and-reencode policy remains unchanged. The outer execution-proof
decoder now rejects trailing bytes and encodings that do not round-trip exactly. Witness hydration,
proof decoding, and witness merging use the fixed `MAX_DEFERRED_ELEMENTS` ceiling. Merged root
sequences use the fixed `MAX_PRECOMPILE_ROOTS` ceiling; low-level decoding still retains its
per-allocation ceiling.

Outer-envelope, file, and network-payload limits remain ingestion concerns.

### Synchronous execution and proving

The FastProcessor-backed `prove_sync(&Prover, ...)` function is the direct synchronous path for
executing and fully proving a program. It preserves the optimized overlapped execution/trace-build
path from PR #3407 when enabled in `ExecutionOptions`; proof-generation policy remains configured on
`Prover`.

## STARK Backend

The prover uses [Plonky3](https://github.com/0xMiden/Plonky3), a modular STARK proving framework.
STARK configurations are defined in the `miden-air` crate and shared between the prover and
verifier, ensuring consistency across the system.

### Hash Function Selection

Different hash functions offer different tradeoffs:
- **BLAKE3/Keccak**: Fast proving but not efficient for recursion
- **RPO256/Poseidon2/RPX256**: Slower proving but efficient for recursive verification in Miden VM

## Crate features
Miden prover can be compiled with the following features:

* `std` - enabled by default and relies on the Rust standard library.
* `concurrent` - implies `std` and also enables multi-threaded proof generation.
* `no_std` does not rely on the Rust standard library and enables compilation to WebAssembly.
    * Only the `wasm32-unknown-unknown` and `wasm32-wasip1` targets are officially supported.

To compile with `no_std`, disable default features via `--no-default-features` flag.

### Concurrent proof generation
When compiled with the `concurrent` feature enabled, the prover generates STARK proofs using
multiple threads. For the benefits of concurrent proof generation, see these
[benchmarks](../README.md#Performance).

Internally, we use [rayon](https://github.com/rayon-rs/rayon) for parallel computations. Use the
`RAYON_NUM_THREADS` environment variable to control the number of threads used to generate a STARK
proof.

## License
This project is dual-licensed under the [MIT](http://opensource.org/licenses/MIT) and
[Apache 2.0](https://opensource.org/license/apache-2-0) licenses.
