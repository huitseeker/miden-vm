# MIRAI annotations

MIRAI checks arithmetic, indexing, panic paths, and function contracts. Run the current analysis
with `make mirai`. The target checks production library manifests separately and runs each AIR
input verifier root in isolation. It fails when MIRAI prints a diagnostic. Analyzer timeout logs
remain visible and need separate review.

The workspace gets `mirai-annotations` from the `main` branch of
`https://github.com/huitseeker/mirai-endorlabs`. Install MIRAI from the same repository and use the
nightly named by `MIRAI_TOOLCHAIN` in the Makefile.

Build the analyzer from the same repository revision recorded for `mirai-annotations` in
`Cargo.lock`. Mixing analyzer and annotation revisions can produce false diagnostics or analyzer
panics.

## AIR input coverage

The handwritten AIR crate defines MIRAI-only verifier roots in
`crates/air/source/src/vm/mirai_air.rs`. Each root tags one prover-committed input column and
evaluates the production constraints for the Core, Chiplets, Eidos compression, or byte-pair
lookup AIR. Separate roots keep MIRAI's tag state small enough to finish. The analysis builder
collects tags only when `assert_zero` or `assert_zero_ext` receives an expression. MIRAI then
verifies that the source tag reached one of those methods.

The sources are current and next-row main trace columns, auxiliary trace columns, and committed
permutation values. Advice enters the main trace and needs no separate source. Public values,
periodic values, preprocessed values, and permutation randomness are verifier-controlled and are
not tagged.

This is a forward data-flow check. It catches a committed input that never reaches an asserted
constraint. It does not prove that a selector activates the constraint when the input is live, so
selector liveness and non-vacuous gating require separate checks.

## Annotation policy

| Situation | Action |
| --- | --- |
| A public function requires valid input | Add a checked precondition and document when the normal build panics. |
| An internal relation follows from the data structure | Add a debug checked assumption next to the operation that needs it. |
| A function establishes a fact that callers need | Add a postcondition after MIRAI can prove it from the function body. |
| Untrusted input can overflow before validation | Use checked arithmetic and return an error. |
| The code can fail for an accepted input | Fix the code. Do not add an assumption. |
| MIRAI proves an assumption without help | Remove the assumption. |

Checked annotations keep a normal assertion in debug or release builds, according to the macro.
Plain `assume!` is not used because it has no runtime check. `assume_preconditions!` is also not
used because it can hide missing contracts in callees.

## Finding ledger

The first pass covered the production libraries listed in `MIRAI_MANIFESTS`. Crates with no MIRAI
diagnostic are kept in the target so later changes cannot add an unchecked panic path.

| Crate and area | Invariant or defect | Treatment |
| --- | --- | --- |
| `miden-serde-utils`, buffered reads | The read cursor stays within the vector, and live buffers fit in allocated capacity. | Debug checked assumptions record the buffer relations. |
| `miden-stark`, missing sibling iterator | The iterator consumes one node, or two nodes when the sibling is present. | A debug checked assumption records the remaining range before advancing it. |
| `miden-crypto`, XChaCha encryption | The authentication tag length must fit when added to the plaintext length. | The public method returns an encryption error when the length cannot be represented. |
| `miden-air-definitions`, recursive quotient chunks | The shift count is smaller than the host word width. | A debug checked assumption records the bound derived from the AIR set. |
| `miden-air-definitions`, lookup flags | The Boolean row decoder and polynomial decoder return the same flags on a valid row. | Debug parity assertions are debug checked assumptions, so MIRAI can use the equality. |
| `miden-core`, operation batches | `add_op` is called only after `can_accept_op`, and the immediate slot is in range. | A checked precondition records the caller duty, and a debug checked assumption guards the slot write. |
| `miden-core`, external MAST nodes | Asking an external node for a domain is an invalid trait operation. | The panic is marked unrecoverable. |
| `miden-core`, basic block decoding | Packed deltas are bounded, and untrusted operation offsets may overflow. | The bounded sum has a debug checked assumption, and the offset uses checked addition. |
| `miden-processor`, advice store | A successful byte budget check also bounds the stored node count. | Debug checked assumptions record the count relation where MIRAI loses the byte conversion. |
| `miden-processor`, trace layout | Component lengths fit in the combined trace and in flat row buffers. | Debug checked assumptions record the additions and the ACE row multiplication. |
| `miden-processor`, ordered traces | Contexts and addresses increase during ordered map iteration. | Debug checked assumptions record the ordering before subtraction. |
| `miden-processor`, counters | Trace counters stay below their integer limits. | Debug checked assumptions record the finite collection bounds. |
| `TraceLenSummary` | Padded heights could be smaller than the unpadded trace, and percentage multiplication could overflow. | The constructor rejects an invalid height, and percentage arithmetic uses `u128`. |

The other production libraries had no MIRAI safety diagnostic in the first pass. They include
`miden-vm-stark`, `miden-ace`, `miden-constraint-compiler`, `miden-assembly-syntax`,
`miden-assembly`, `miden-verifier`, `miden-prover`, and `miden-vm`.

## Incomplete analysis

A MIRAI run can finish successfully after timing out in some function bodies. A timeout means the
function was not fully checked. It is not the same as a clean proof.

| Area | Common timeout source | Next step |
| --- | --- | --- |
| Cryptography | Ed25519 and Curve25519 dependency code | Add trusted summaries at the dependency boundary, or analyze those crates separately. |
| AIR construction | Generic constraint builders and iterator collection | Add small postconditions to Miden helpers before adding summaries for generic iterators. |
| MAST and advice data | Collection, canonical ordering, and deserialization paths | Split large helpers and state the size relations returned by each step. |
| Processor and prover | Rayon iterator plumbing and trace construction | Analyze sequential helpers first, then summarize the parallel adapter layer. |

When a timeout disappears, inspect the newly analyzed body for diagnostics before removing it from
the table. Do not treat a lower timeout count by itself as a quality improvement.
