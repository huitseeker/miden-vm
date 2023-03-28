use super::build_test;
use miden::{crypto::Rpo256, math::Felt};
use vm_core::{
    crypto::merkle::{MerkleStore, NodeIndex},
    utils::IntoBytes,
    StarkField, Word, WORD_SIZE,
};

#[test]
fn mtree_get_works_for_subtree() {
    let source = r#"
        begin
            mtree_get
        end
    "#;

    // setup the store
    let depth = 48;
    let index = 0xfffffffffff1;
    let value = [Felt::new(199); WORD_SIZE];
    let mut store = MerkleStore::new();
    let root = store.add_sparse_merkle_tree(depth, [(index, value)]).unwrap();

    // setup the target sub-tree root
    let root_depth = 16;
    let root_index = index << (depth - root_depth);
    let root_value = store.get_node(root, NodeIndex::new(root_depth, root_index)).unwrap();

    // setup the target sub-tree node
    // a relative sub-tree index of the previous tier is calculated via `i - 2^T * floor(i / 2^T)`
    let node_depth = 32;
    let node_index = index << (depth - node_depth);
    let node_relative_depth = 16;
    let node_relative_index = node_index - (1 << 16) * (node_index >> 16);
    let node_value = store.get_node(root, NodeIndex::new(node_depth, node_index)).unwrap();

    // set the initial stack (reversed order) and build the test
    let initial_stack = [
        root_value[0].as_int(),
        root_value[1].as_int(),
        root_value[2].as_int(),
        root_value[3].as_int(),
        node_relative_index,
        node_relative_depth,
    ];
    let advice_stack = [];
    let test = build_test!(source, &initial_stack, &advice_stack, store);

    // set the expected result
    let expected_output = [
        node_value[3].as_int(),
        node_value[2].as_int(),
        node_value[1].as_int(),
        node_value[0].as_int(),
        root_value[3].as_int(),
        root_value[2].as_int(),
        root_value[1].as_int(),
        root_value[0].as_int(),
    ];

    // assert the stack equals the expected result
    test.expect_stack(&expected_output);
}

#[test]
fn smtget_opens_correctly() {
    let seed = blake3::hash(b"some-seed");

    // compute pseudo-random key/value pair
    let key = <[u8; 8]>::try_from(&seed.as_bytes()[..8]).unwrap();
    let key = u64::from_le_bytes(key);
    let key = [Felt::new(key); WORD_SIZE];
    let value = <[u8; 8]>::try_from(&seed.as_bytes()[8..16]).unwrap();
    let value = u64::from_le_bytes(value);
    let value = [Felt::new(value); WORD_SIZE];

    fn assert_case(
        depth: u64,
        index: u64,
        key: Word,
        value: Word,
        node: Word,
        root: Word,
        store: MerkleStore,
    ) {
        let source = r#"
            begin
                adv.smtget
                dropw
                dropw
                adv_push.10
            end
        "#;
        let initial_stack = [
            root[0].as_int(),
            root[1].as_int(),
            root[2].as_int(),
            root[3].as_int(),
            key[0].as_int(),
            key[1].as_int(),
            key[2].as_int(),
            key[3].as_int(),
        ];
        let expected_output = [
            node[3].as_int(),
            node[2].as_int(),
            node[1].as_int(),
            node[0].as_int(),
            value[3].as_int(),
            value[2].as_int(),
            value[1].as_int(),
            value[0].as_int(),
            depth,
            index,
        ];
        let advice_stack = [];
        let advice_map = [(key.to_owned().into_bytes(), value.to_vec())];
        build_test!(source, &initial_stack, &advice_stack, store.clone(), advice_map)
            .expect_stack(&expected_output);

        let source = r#"
            use.std::collections::smt

            begin
                exec.smt::get
            end
        "#;
        let initial_stack = [
            root[0].as_int(),
            root[1].as_int(),
            root[2].as_int(),
            root[3].as_int(),
            key[0].as_int(),
            key[1].as_int(),
            key[2].as_int(),
            key[3].as_int(),
        ];
        let expected_output = [
            depth,
            index,
            value[3].as_int(),
            value[2].as_int(),
            value[1].as_int(),
            value[0].as_int(),
            key[3].as_int(),
            key[2].as_int(),
            key[1].as_int(),
            key[0].as_int(),
            root[3].as_int(),
            root[2].as_int(),
            root[1].as_int(),
            root[0].as_int(),
        ];
        let advice_stack = [];
        let advice_map = [(key.to_owned().into_bytes(), value.to_vec())];
        build_test!(source, &initial_stack, &advice_stack, store, advice_map)
            .expect_stack(&expected_output);
    }

    // insert the leaf on the first tier
    let depth = 16;
    let index = key[3].as_int() >> 48;
    let mut store = MerkleStore::new();
    let node = Rpo256::merge_in_domain(&[key.into(), value.into()], Felt::new(depth)).into();
    let root = store.add_sparse_merkle_tree(depth as u8, [(index, node)]).unwrap();
    assert_case(depth, index, key, value, node, root, store);

    // insert the leaf on the second tier
    let depth = 32;
    let index = key[3].as_int() >> 32;
    let mut store = MerkleStore::new();
    let node = Rpo256::merge_in_domain(&[key.into(), value.into()], Felt::new(depth)).into();
    let root = store.add_sparse_merkle_tree(depth as u8, [(index, node)]).unwrap();
    assert_case(depth, index, key, value, node, root, store);

    // insert the leaf on the third tier
    let depth = 48;
    let index = key[3].as_int() >> 16;
    let mut store = MerkleStore::new();
    let node = Rpo256::merge_in_domain(&[key.into(), value.into()], Felt::new(depth)).into();
    let root = store.add_sparse_merkle_tree(depth as u8, [(index, node)]).unwrap();
    assert_case(depth, index, key, value, node, root, store);

    // insert the leaf on the last tier
    let depth = 64;
    let index = key[3].as_int();
    let mut store = MerkleStore::new();
    let node = Rpo256::merge_in_domain(&[key.into(), value.into()], Felt::new(depth)).into();
    let root = store.add_sparse_merkle_tree(depth as u8, [(index, node)]).unwrap();
    assert_case(depth, index, key, value, node, root, store);
}
