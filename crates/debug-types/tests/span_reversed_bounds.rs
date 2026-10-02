//! Reproduction for: `SourceSpan` wire decoder accepts reversed bounds (`start > end`);
//! the decoded value panics on `.len()`.
//!
//! Issue: https://github.com/0xMiden/miden-vm/issues/3962

use miden_crypto::utils::Deserializable;
use miden_debug_types::SourceSpan;

/// A payload of `source_id = 0, start = 5, end = 3` (three little-endian `u32`s) decodes
/// successfully, although `SourceSpan`'s invariant is `start <= end` and the writer can
/// never emit a reversed span.
#[test]
fn reversed_bounds_decode_successfully() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0u32.to_le_bytes()); // source_id
    bytes.extend_from_slice(&5u32.to_le_bytes()); // start
    bytes.extend_from_slice(&3u32.to_le_bytes()); // end (reversed)

    let span = SourceSpan::read_from_bytes(&bytes)
        .expect("reversed bounds decode successfully");
    assert_eq!(span.start().to_usize(), 5);
    assert_eq!(span.end().to_usize(), 3);
}

/// Downstream consequence: the decoded reversed span panics on `.len()`, which computes
/// the unsigned difference `end - start`. Debug builds only; release wraps.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "attempt to subtract with overflow")]
fn decoded_reversed_span_panics_on_len() {
    use miden_crypto::utils::Deserializable;
    use miden_debug_types::SourceSpan;

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&5u32.to_le_bytes());
    bytes.extend_from_slice(&3u32.to_le_bytes());

    let span = SourceSpan::read_from_bytes(&bytes).unwrap();
    let _ = span.len();
}
