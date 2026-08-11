---
title: "Deferred state semantics and API contract"
sidebar_position: 2
---

# Deferred state semantics and API contract

`DeferredState` is the host-side witness for deferred DAG verification and the deferred root
commitment. It is not cryptographic evidence by itself. VM proving wraps non-empty state as a
singleton `PrecompileWitness` retained by `ExecutionProof::Deferred`; completing the proof replaces
that witness with a compatible `PrecompileProof`.

The simplified state model is:

```rust
pub struct DeferredState {
    registry: Arc<PrecompileRegistry>,
    nodes: BTreeMap<Digest, Node>,
    root: Digest,
    remaining_elements: usize,
    // evaluation results may be memoized internally, but this is not part of the public contract
}
```

## Vocabulary

- **Registered** means a digest has an entry in `DeferredState.nodes`. Registration can happen
  through `DeferredState::register`, evaluation storing canonical/helper nodes, `log_statement`
  storing framework `AND` nodes, or wire rehydration rebuilding entries.
- **Evaluated** means a registered input digest has been semantically reduced to a canonical node
  under the installed `PrecompileRegistry`. The canonical node is also stored in `nodes` so it can
  be referenced by downstream nodes.
- **Logged** or **root-reachable** means a registered digest contributes to `DeferredState.root`.
  Only the root-reachable closure is serialized by `to_wire`; registered/evaluated orphans are
  dropped.

## Registered nodes

`nodes` is the durable node store.

- `TRUE_DIGEST` is always present and maps to `Node::TRUE`.
- `Node::TRUE` costs no budget.
- Every non-TRUE node is keyed by `node.digest()`.
- Structural nodes may reference only children already present in `nodes`, except for the implicit
  `TRUE_DIGEST`:
  - `Join` has two child digests.
  - `PairList` has one or more pairs of child digests.
- Re-registering identical content is idempotent and free.
- Reusing an existing digest for different content is rejected as a conflicting node.

Registration stores and shape-checks a node in `nodes`, evaluates it immediately, and stores the
canonical result. False predicates and other semantic evaluation failures are reported by
registration.

## One fixed ceiling

`DeferredState::new(registry)` initializes one total budget from the library safety ceiling:

```text
remaining_elements = MAX_DEFERRED_ELEMENTS
```

Initialization also installs the registry's `init()` constants, charging them against that same
budget. `extend_precompiles(precompiles)` merges additional precompiles into an existing state
without discarding existing nodes, evaluation results, root, or budget accounting.

Every new unique durable node inserted into `nodes` decrements `remaining_elements` by the node's
field-element footprint using checked subtraction. Duplicate insertion is free, so registering the
same data node at the exact budget limit succeeds. Evaluation results do not have a separate budget
and do not double-count canonical payloads; only canonical/helper nodes newly inserted into `nodes`
are charged.

The precompile's `decode` result is the framework shape gate:

- `NodeType::Data` authorizes a non-empty data payload. For memory-backed registration, the host
  reads exactly the stack-supplied `n_chunks`; precompile evaluation checks any tag-derived semantic
  data length.
- `NodeType::Join` authorizes exactly one 8-felt payload block, interpreted as two child digests.
- `NodeType::PairList` authorizes a non-empty list of `lhs_digest || rhs_digest` chunks. Precompile
  evaluation checks any tag-derived semantic pair count.

Processor handlers perform a cheap deferred-budget pre-check before allocating or reading a
memory-backed payload, but exact data/pair-list arity remains precompile-specific semantics.

If insertion exhausts the remaining budget, execution aborts with a budget error. The insertion path
owns this accounting; processor deferred handlers do not perform post-mutation deferred budget
checks.

## Evaluation

Evaluation first requires the input digest to be present in `nodes`; evaluation state alone never
creates durable DAG membership. A call to `evaluate_digest(digest)` returns the digest of the
canonical node. This is a semantic operation: it may compute the result or use internal
memoization, but callers do not observe that distinction. Callers that need canonical node contents
can compose `evaluate_digest` with `get_node`.

Framework nodes evaluate as follows:

```text
Node::TRUE => Node::TRUE
Node::AND(lhs, rhs) =>
  require evaluate_digest(lhs) == TRUE_DIGEST
  require evaluate_digest(rhs) == TRUE_DIGEST
  Node::TRUE
```

Precompile-owned nodes are evaluated by `PrecompileRegistry::evaluate`, which dispatches to the
owning `Precompile` with a `DeferredContext`.

`DeferredContext` gives precompile implementations the same semantic split:

- `get_node(digest)` queries the registered/original node by digest without evaluating it.
- `evaluate_digest(digest)` evaluates a registered child digest to its canonical digest.
- `evaluate_digest_pair(lhs, rhs)` evaluates two registered child digests to canonical digests.
- `ensure_equal(lhs, rhs)` evaluates two children and requires their canonical digests to match.
- `register(node)` inserts a freshly minted helper node and returns its original digest.

## Root and wire

`root` starts at `TRUE_DIGEST`. `log_statement(stmt_digest)` evaluates the current root and
statement, requires both to evaluate to `Node::TRUE`, then appends one framework `AND` node:

```text
next_root = digest(Node::and(previous_root, stmt_digest))
```

`to_wire` serializes only the root-reachable closure in canonical child-first order:

- data entries carry literal data chunks;
- join entries emit two child indices;
- pair-list entries emit pairs of child indices.

The wire root is implicit: empty wire opens `TRUE_DIGEST`, otherwise the root is the digest of the
final entry. `from_wire(registry, wire)` decodes untrusted wire under `MAX_DEFERRED_ELEMENTS`,
rejects non-canonical or dangling wire by requiring `state.to_wire() == wire`, then evaluates the
implicit wire root to `Node::TRUE`. Evaluation may insert canonical/helper nodes in addition to the
wire nodes.

## Proof obligations and composition

`Prover::prove` consumes an `ExecutionWitness` and proves its VM portion. If the authenticated root
is `TRUE_DIGEST`, it returns `ExecutionProof::Complete` without a precompile proof. Otherwise it
returns `ExecutionProof::Deferred`, pairing the `VmProof` with the matching singleton
`PrecompileWitness`. `Prover::prove_full` proves both stages in memory and returns `Complete`.

There are two delegated workflows. To complete a VM-first deferred proof, borrow its witness with
`ExecutionProof::precompile_witness()` and pass it by reference to
`Prover::prove_precompile`; clone only when ownership-requiring transport needs a separate value. To
delegate VM and precompile proving independently, split the original `ExecutionWitness` with
`into_parts()` before either worker proves its artifact. The resulting `PrecompileProof` retains the
ordered, non-empty constituent roots covered by its aggregate STARK. `ExecutionProof::complete`
checks that the artifact covers the VM-authenticated root and transitions the proof from `Deferred`
to `Complete` without reproving the VM.

`PrecompileWitness::merge` accepts a non-empty sequence of singleton witnesses only. For example,
merging `[one, one, two]` preserves that exact root order and both occurrences of `one`; the
aggregate root is their ordered framework-`AND` fold. A multi-root merged witness is not a singleton
and cannot be merged again; a one-input merge remains singleton. The input list is bounded by
`MAX_PRECOMPILE_ROOTS`, and the entire merged state is bounded by `MAX_DEFERRED_ELEMENTS`. A
`PrecompileProof` produced from the merged witness can be reused
to complete compatible individual deferred proofs for `one` or `two`, because completion checks
whether each individual root is covered in order. This reuse does not introduce a multi-execution
proof artifact or specify a future settlement envelope.

`ExecutionProof::new_deferred`, `ExecutionProof::new_complete`, and `ExecutionProof::complete` check
artifact structure only; public variants may bypass those conveniences. Encoding and decoding also
do not establish validity. `Verifier::verify` revalidates structure, verifies the VM STARK for both
lifecycle states, and verifies the aggregate precompile STARK for a complete proof that carries one.
A deferred outcome returns the exact authenticated outstanding root. Callers that require settlement
inspect `VerificationOutcome::is_complete()` after verification rather than trusting
`ExecutionProof::is_complete()` before it.

`DeferredStateWire` remains the low-level canonical representation used to serialize hydrated
witness material inside a deferred proof. `PrecompileWitness` may contain private execution data and
a large hydrated DAG; treat it as sensitive prover input and borrow it during proving.

## Low-level framework API

The preferred low-level `miden_core::deferred::DeferredState` surface is small. These APIs are
framework APIs, not the public proof-verifier policy surface:

- `DeferredState::new(registry)` for a state booted with precompile constants and the fixed ceiling
- `extend_precompiles(precompiles)` for additive setup
- `registry()`
- `root()`
- `remaining_elements()`
- `get_node(digest)` and `nodes() -> &BTreeMap<Digest, Node>` for registered-node inspection
- `decode(tag)` for structural tag decoding
- `register(node)` for inserting concrete node content
- `evaluate_digest(digest)` for the canonical digest
- `log_statement(stmt_digest)`
- `to_wire()`
- `from_wire(registry, wire)`

Callers that have a concrete node should explicitly `register` it; they may call
`evaluate_digest` on the returned digest when they need the canonical result, and then `get_node` if
they need canonical node contents. Raw evaluation memoization and direct root mutation are not part
of the public contract.

## Scope note

Deferred witness decoding is context-aware: `DeferredState::from_wire`,
`PrecompileWitness::from_bytes`, and `ExecutionProof::read_from_bytes` require callers to supply a
trusted `PrecompileRegistry`. Witness hydration and execution-proof decoding use the fixed
`MAX_DEFERRED_ELEMENTS` ceiling, including the lower-level `DeferredState::from_wire` path. For
bundled precompiles, ordinary callers use
`miden_vm::read_execution_proof_from_bytes`, which installs the standard registry. Custom-registry
callers retain the low-level proof decoder;
`PrecompileRegistry::new()` is an empty registry. Neither `Prover` nor `Verifier` configuration
supports caller-selected registries.

The deferred-wire canonical decode-and-reencode check remains unchanged. The outer
`ExecutionProof` decoder now also rejects trailing bytes and encodings that do not round-trip
exactly. Decoding retains a separate per-allocation ceiling. State and witness hydration,
execution-proof decoding, and witness merging use the fixed `MAX_DEFERRED_ELEMENTS` ceiling, while
merged constituent roots use `MAX_PRECOMPILE_ROOTS`. Outer-envelope, file, and network-payload
limits remain ingestion concerns.
