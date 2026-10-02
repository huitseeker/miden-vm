//! Reproduction for: the `UniqueNodes` wire reader (partial SMT serialization) accepts
//! non-canonical and self-contradictory payloads that the writer can never produce.
//!
//! Issue: https://github.com/0xMiden/miden-vm/issues/3965

use miden_crypto::merkle::{NodeIndex, smt::{LeafIndex, SMT_DEPTH, SmtLeaf, UniqueNodes}};
use miden_crypto::{Felt, Word};
use miden_crypto::utils::{ByteWriter, Deserializable, Serializable};

/// Hand-writes the `UniqueNodes` node-section payload: an expected root, then `levels`
/// as `(depth, [(position, value)])` with an explicit depth per level and per-node
/// values, then an empty leaf section and an empty value-only leaf section.
fn write_nodes_payload(levels: &[(u8, &[(u64, Word)])], target: &mut Vec<u8>) {
    Word::default().write_into(target); // expected root
    target.write_u64(levels.len() as u64);
    for (depth, nodes) in levels {
        target.write_u8(*depth);
        target.write_u64(nodes.len() as u64);
        for (position, value) in nodes.iter().copied() {
            target.write_u64(position);
            value.write_into(target);
        }
    }
    target.write_u64(0); // leaf count
    target.write_u64(0); // value-only leaf count
}

/// Part 1: a node position repeated WITHIN one level is silently overwritten (last value
/// wins) instead of rejected. `write_into` can never produce this payload: the `BTreeMap`
/// source cannot hold duplicate keys.
#[test]
fn duplicate_node_position_in_a_level_is_silently_overwritten() {
    let first_value = Word::new([Felt::new_unchecked(0x6700_0000_0000_0001); 4]);
    let second_value = Word::new([Felt::new_unchecked(0x6700_0000_0000_0002); 4]);
    let mut bytes = Vec::new();
    write_nodes_payload(&[(1, &[(1, first_value), (1, second_value)])], &mut bytes);

    let decoded = UniqueNodes::read_from_bytes(&bytes)
        .expect("a repeated node position decodes successfully");
    let node = decoded.nodes.get(&NodeIndex::new(1, 1).unwrap()).unwrap();
    assert_eq!(node, &second_value, "the LAST duplicate wins");
    assert_ne!(node, &first_value, "keep-first must not pass");
}

/// Part 2: levels are accepted in any depth order, so the same node set has multiple
/// valid byte encodings — the wire form is not uniquely decodable. The writer emits
/// ascending depths (BTreeMap iteration order); this payload is the reverse.
#[test]
fn levels_in_descending_order_are_accepted() {
    let mut bytes = Vec::new();
    let word_at = |i: u64| Word::new([Felt::new_unchecked(0x6700_0000_0000_0000 + i); 4]);
    write_nodes_payload(&[(2, &[(1, word_at(1))]), (1, &[(1, word_at(2))])], &mut bytes);

    UniqueNodes::read_from_bytes(&bytes)
        .expect("descending depth levels decode successfully");
}

/// Part 3: a position present in BOTH `leaves` and `value_only_leaves` decodes
/// successfully, and `get_leaf_hash` silently shadows the value-only entry with the
/// leaf's hash. The construction path can never produce the overlap — a position becomes
/// either a leaf or a value-only entry, exclusively.
#[test]
fn position_in_leaves_and_value_only_leaves_is_accepted_and_shadowed() {
    // A POPULATED leaf: its hash differs from EMPTY_WORD (the missing-leaf fallback), so
    // the shadowing assertion cannot pass under a get_leaf_hash that ignores both maps.
    let key = Word::new([Felt::new_unchecked(0x6700_0000_0000_0009); 4]);
    let leaf_value = Word::new([Felt::new_unchecked(0x6700_0000_0000_000a); 4]);
    let leaf_index = LeafIndex::<SMT_DEPTH>::from(key);
    let leaf = SmtLeaf::new(vec![(key, leaf_value)], leaf_index).expect("populated leaf is valid");
    let position = leaf_index.position();
    let shadowed_value = Word::new([Felt::new_unchecked(0x6700_0000_0000_0003); 4]);

    let mut bytes = Vec::new();
    Word::default().write_into(&mut bytes); // expected root
    bytes.write_u64(0); // level count
    bytes.write_u64(1); // leaf count
    position.write_into(&mut bytes); // the leaf section's position field
    leaf.write_into(&mut bytes); // num_entries + embedded leaf index + entries
    bytes.write_u64(1); // value-only leaf count
    position.write_into(&mut bytes);
    shadowed_value.write_into(&mut bytes);

    let decoded = UniqueNodes::read_from_bytes(&bytes)
        .expect("an overlapped position decodes successfully");

    // The exact decoded state in both maps.
    assert_eq!(decoded.leaves.get(&position), Some(&leaf), "leaf present verbatim");
    assert_eq!(
        decoded.value_only_leaves.get(&position),
        Some(&shadowed_value),
        "value-only entry present for the SAME position",
    );

    // Leaves take priority: the lookup returns the POPULATED leaf's hash, which differs
    // from both EMPTY_WORD (the missing-leaf fallback) and the shadowed value — so this
    // establishes leaves-first selection, not just overlap acceptance.
    assert_ne!(leaf.hash(), Word::default(), "populated leaf must not hash to EMPTY_WORD");
    assert_eq!(
        decoded.get_leaf_hash(position),
        leaf.hash(),
        "the leaf silently shadows the value-only entry",
    );
    assert_ne!(decoded.get_leaf_hash(position), shadowed_value);
    assert_ne!(decoded.get_leaf_hash(position), Word::default());
}
