# Precompiles

Precompiles let Miden programs make claims about expensive computations without executing them
directly in the VM trace, while still binding those claims into the VM proof. This page covers the
VM-side mechanics: wrappers register deferred nodes, bind their digests to circuit-visible data, and
log statement digests that evaluate to `TRUE`. `VmProof` authenticates the resulting root;
`ExecutionProof::Deferred` retains the witness for a non-empty obligation, and a compatible
`PrecompileProof` transitions it to `ExecutionProof::Complete`. For proof shapes, see
[Deferred computation](../deferred/index.md).

Concrete proof-bound implementations live in the `miden-precompiles` crate. Their MASM support
modules are currently internal implementation detail used by core-library facades and tests.

## Current data model

- **`Tag`** — A 4-felt node constructor. Framework ids `0`, `1`, and `2` are reserved for `TRUE`,
  semantic `AND`, and opaque framework `CHUNKS`. Precompile ids are derived from precompile names
  and interpret the remaining three `args` felts locally.
- **`Node`** — A content-addressed `(tag, payload)` term in the deferred DAG. Payloads are data
  chunks, join child digests, pair lists of `lhs_digest || rhs_digest` chunks, or the framework
  `TRUE` sentinel.
- **`Precompile`** — A host implementation that owns one precompile id and decodes the structural
  shape for its tags. It evaluates nodes to canonical form and optionally contributes constants
  through `init()`.
- **`PrecompileRegistry`** — The host/framework dispatcher for trusted precompile implementations.
  The type remains in `miden-core` so the framework does not depend on concrete implementations.
  `PrecompileRegistry::new()` is an empty low-level registry; callers decoding bundled precompiles
  normally use `miden_precompiles::registry()`.
- **`DeferredState`** — The host-side DAG witness accumulated during execution. It tracks
  registered nodes, evaluates them under the registry, and maintains the rolling deferred root.
- **`DeferredStateWire`** — A low-level canonical witness transport format. It is passive data
  until rehydrated and validated with an explicit registry. Standard witness hydration and proof
  decoding use the fixed `MAX_DEFERRED_ELEMENTS` ceiling. Deferred proofs use the wire to serialize
  retained witness material; the wire is not cryptographic evidence.
- **Deferred root** — A single digest public value. Each logged statement appends
  `Node::AND(previous_root, statement_digest)` and advances the root to that node digest.

## Lifecycle overview

1. **Wrapper registers nodes** – Internal MASM support code stages node payloads on the operand
   stack or in memory and emits `adv.register_deferred` / `adv.register_deferred_data`.
   Registration stores the node in host-side `DeferredState`, checks structural child closure, and
   evaluates the node immediately under the installed registry.
2. **Wrapper binds digests inside the VM** – Registration arguments are visible in the VM
   execution trace, but the event does not constrain the host-side `DeferredState` update.
   Memory-backed registration also performs direct host reads without adding AIR accesses. The
   wrapper computes each proof-relevant digest with VM instructions from the exact same tag and
   stack payload or ordered memory chunk sequence.
3. **Wrapper evaluates only through explicit predicates** – When a wrapper uses
   `adv.evaluate_deferred*` to obtain host-computed canonical data, it must use VM instructions to
   relate that advice to values established independently of it, then log a statement digest that
   the verifier can re-evaluate.
4. **`log_deferred` folds a statement** – The opcode expects `STMNT` at stack offsets `4..8`.
   `STMNT` must already be registered in `DeferredState` and evaluate to `TRUE`. The constrained
   Poseidon2 permutation computes `ROOT_NEW = rate0(Poseidon2([ROOT_PREV, STMNT, Tag::AND]))`, and
   host-side deferred state records the corresponding `AND` node.
5. **Prover binds the deferred root** – `Prover::prove` proves the VM and returns
   `ExecutionProof::Complete` when the root is `TRUE_DIGEST`; otherwise it returns
   `ExecutionProof::Deferred` with the matching singleton `PrecompileWitness`. `Prover::prove_full`
   proves both stages in memory and always returns `Complete`.
6. **Precompile proving can be delegated** – To complete a VM-first deferred proof, borrow its
   witness through `precompile_witness()` and pass it by reference to
   `Prover::prove_precompile`; clone only when transport needs ownership. To delegate both proving
   stages independently, split `ExecutionWitness` before sending either witness to a worker.
   `PrecompileWitness::merge([one, one, two])` preserves that order and duplicate multiplicity, but
   only singleton inputs are accepted. The input count is capped at `MAX_PRECOMPILE_ROOTS`, and the
   complete merged state is capped at `MAX_DEFERRED_ELEMENTS`. A multi-root merged witness
   cannot be merged again; a one-input merge remains singleton. One resulting `PrecompileProof` may
   complete compatible individual deferred proofs; this is artifact reuse, not a batch settlement
   envelope.
7. **Verifier authenticates available evidence** – `ExecutionProof::complete` attaches a
   compatible precompile proof without reproving the VM. `Verifier::verify` revalidates structure
   and accepts both lifecycle states: it verifies the VM STARK for `Deferred` and the precompile
   STARK for `Complete` when present. Deferred outcomes expose the exact authenticated outstanding
   root. Finality-sensitive callers inspect `VerificationOutcome::is_complete()`.

## Responsibilities

- **VM** — Executes deferred advice events and `log_deferred`, maintains the rolling deferred
  root, and exposes the final root as a public value.
- **Host / advice provider** — Maintains `DeferredState`, runs trusted precompile implementations,
  and supplies evaluation advice when wrappers request it.
- **MASM wrapper** — Registers concrete deferred nodes and computes node and statement digests
  with VM instructions from exact stack payloads or memory reads. It logs only statements that
  should evaluate to `TRUE`, and hides helper outputs from callers when appropriate.
- **Prover** — Produces the `VmProof`, retains a singleton `PrecompileWitness` in a deferred
  proof, and produces a singular `PrecompileProof` from one singleton or one ordered merge.
  Witnesses may be private and large, so precompile proving borrows the hydrated witness.
- **Verifier** — Verifies the VM proof in either lifecycle state and, for a complete proof that
  carries one, verifies the precompile proof and its ordered root coverage.

## Registry and transport policy

Ordinary façade callers decode with `miden_vm::read_execution_proof_from_bytes`, which installs the
standard bundled registry. Custom-precompile callers use `ExecutionProof::read_from_bytes` or the
lower-level witness decoder with an explicit trusted registry; both use the fixed
`MAX_DEFERRED_ELEMENTS` ceiling. `DeferredState::from_wire` uses that same fixed ceiling. `Prover`
and `Verifier` do not support caller-selected registries.

Encoding preserves representation and does not establish proof validity. The deferred-wire
canonical re-encode check remains, while the outer execution-proof decoder now rejects trailing
bytes and non-round-tripping encodings. Witness hydration, execution-proof decoding, and witness
merging use the same fixed `MAX_DEFERRED_ELEMENTS` ceiling; root sequences use
`MAX_PRECOMPILE_ROOTS`. Outer-envelope, file, and network-payload limits remain ingestion concerns.
Witness merging enables proof reuse, not settlement.

## Conventions

- Tag layout: `TAG = [precompile_id, arg0, arg1, arg2]`.
  - `precompile_id` selects the framework or owning precompile.
  - `arg0..arg2` are interpreted by the selected precompile.
  - Framework id `0` is `Tag::TRUE`; framework id `1` is `Tag::AND`; framework id `2` is
    `Tag::CHUNKS`.
- Payload shapes are declared by the selected precompile's `decode(args)`, but semantic lengths are
  tag-specific and validated by the owning precompile:
  - `NodeType::Data` accepts one or more opaque 8-felt chunks. For memory-backed registration,
    the stack-supplied `n_chunks` determines how many chunks are read.
  - `NodeType::Join` reads `lhs_digest || rhs_digest`.
  - `NodeType::PairList` accepts one or more `lhs_digest || rhs_digest` chunks. Precompiles that
    encode a pair count in tag arguments must check the actual payload length during evaluation.
- `log_deferred` stack effect: `[_, STMNT, _, ...] -> [ROOT_NEW, OUT_RATE1, OUT_CAP, ...]` where
  `STMNT` occupies stack offsets `4..8`. Wrappers usually drop the three output words after the root
  transition has been constrained.
- Input and memory layouts are precompile-specific. Core-library wrappers define the native formats
  for hash facades and for arithmetic/curve support used by signature verification.

## Examples

- Hash support wrappers register the input/result nodes needed for the hash claim and log a
  statement digest that verifies the claimed digest.
- Signature support wrappers register the public key, precompile-specific message input, signature,
  and verification predicate nodes, then log the predicate statement.


## Related reading

- [Deferred computation](../deferred/index.md) – deferred DAG, witness, and proof artifact model.
- [`log_deferred` instruction](../../user_docs/assembly/instruction_reference.md) – stack
  behaviour and opcode semantics.
- `DeferredStateWire` implementation (`core/src/deferred/wire.rs`) – low-level deferred witness
  transport details.
