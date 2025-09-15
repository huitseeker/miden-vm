use alloc::vec::Vec;
use crate::{
    Decorator, Operation,
    mast::{MastForest, BasicBlockNode, node::MastNodeExt},
    utils::{Serializable, Deserializable, SliceReader},
};

#[test]
fn basic_block_node_roundtrip_serialization() {
    // Test cases covering different scenarios:
    // 1. Operations with padding requiring decorator adjustment
    // 2. Operations with decorators at different positions
    // 3. Operations with immediate values
    let test_cases = vec![
        // Case 1: Simple operations with decorators before and after padding
        (
            vec![
                Operation::Add,
                Operation::Push(crate::ONE),
                Operation::Mul,
                Operation::Push(crate::Felt::new(42)),
                Operation::Drop,
                Operation::MovDn2,
                Operation::Pad,
                Operation::Swap,
                Operation::Dup1,
            ],
            vec![
                (0, Decorator::Trace(0)),      // Before first operation (should be adjusted for padding)
                (1, Decorator::Trace(1)),      // After Push(ONE)
                (4, Decorator::Trace(2)),      // After Drop (affected by padding)
                (9, Decorator::Trace(3)),      // After last operation
            ]
        ),
        // Case 2: Operations requiring multiple batches (forces padding)
        (
            (0..73).map(|i| Operation::Push(crate::Felt::new(i))).collect(),
            vec![
                (0, Decorator::Trace(0)),      // Before first Push
                (72, Decorator::Trace(1)),     // After last Push (adjusted for padding)
            ]
        ),
        // Case 3: Operations with immediate values causing padding
        (
            vec![
                Operation::Push(crate::ONE),
                Operation::Push(crate::Felt::new(2)),
                Operation::Push(crate::Felt::new(3)),
                Operation::Push(crate::Felt::new(4)),
                Operation::Push(crate::Felt::new(5)),
                Operation::Push(crate::Felt::new(6)),
                Operation::Push(crate::Felt::new(7)),
                Operation::Push(crate::Felt::new(8)),
                Operation::Push(crate::Felt::new(9)),  // This will cause padding
                Operation::Add,
            ],
            vec![
                (0, Decorator::Trace(0)),      // Before first Push
                (8, Decorator::Trace(1)),      // Before 9th Push (affected by padding)
                (10, Decorator::Trace(2)),     // After Add (affected by padding from Push operations)
            ]
        ),
        // Case 4: No decorators (edge case)
        (
            vec![Operation::Add, Operation::Mul, Operation::Drop],
            vec![]
        ),
    ];

    for (operations, decorators) in test_cases {
        let mut mast_forest = MastForest::new();

        // Create the original BasicBlockNode
        let original_block = BasicBlockNode::new_with_raw_decorators(
            operations.clone(),
            decorators.clone(),
            &mut mast_forest,
        ).unwrap();

        // Create a new MastForest with the original node
        let mut serialized_forest = MastForest::new();
        let original_node_id = serialized_forest.add_block(operations.clone(), Some(
            decorators.iter().map(|(idx, dec)| (*idx, mast_forest.add_decorator(dec.clone()).unwrap())).collect()
        )).unwrap();

        serialized_forest.make_root(original_node_id);

        // Serialize the MastForest
        let serialized_bytes = serialized_forest.to_bytes();

        // Deserialize
        let mut reader = SliceReader::new(&serialized_bytes);
        let deserialized_forest = MastForest::read_from(&mut reader).unwrap();
        let deserialized_node_id = deserialized_forest.procedure_roots()[0];
        let deserialized_block = match &deserialized_forest[deserialized_node_id] {
            crate::mast::MastNode::Block(block) => block.clone(),
            _ => panic!("Expected basic block node"),
        };

        // Verify the round-trip worked correctly
        assert_eq!(original_block.digest(), deserialized_block.digest());
        assert_eq!(original_block.num_operations(), deserialized_block.num_operations());
        assert_eq!(original_block.decorators().len(), deserialized_block.decorators().len());

        // Compare raw operations (unpadded)
        let original_raw_ops: Vec<_> = original_block.raw_operations().collect();
        let deserialized_raw_ops: Vec<_> = deserialized_block.raw_operations().collect();
        assert_eq!(original_raw_ops, deserialized_raw_ops);

        // Compare decorators with their adjusted positions
        let original_decorators: Vec<_> = original_block.decorators().iter().collect();
        let deserialized_decorators: Vec<_> = deserialized_block.decorators().iter().collect();
        assert_eq!(original_decorators, deserialized_decorators);

        // Test that the padded operations match
        let original_padded_ops: Vec<_> = original_block.operations().collect();
        let deserialized_padded_ops: Vec<_> = deserialized_block.operations().collect();
        assert_eq!(original_padded_ops, deserialized_padded_ops);

        // Test decorator iteration works correctly
        let mut original_iter = original_block.raw_decorator_iter();
        let mut deserialized_iter = deserialized_block.raw_decorator_iter();

        while let Some(orig_dec) = original_iter.next() {
            let deser_dec = deserialized_iter.next();
            assert_eq!(deser_dec, Some(orig_dec), "Mismatch in raw decorator iteration");
        }

        assert_eq!(deserialized_iter.next(), None, "Extra decorators in deserialized block");
    }
}

#[test]
fn decorator_padding_offset_calculation() {
    // Test specific cases to verify decorator offset calculation matches expectations

    // Case 1: Operations that will cause padding in the middle
    let operations = vec![
        Operation::Add,
        Operation::Mul,
        Operation::Push(crate::ONE),    // This will cause padding
        Operation::Push(crate::Felt::new(2)),
        Operation::Drop,
    ];

    let mut mast_forest = MastForest::new();
    let decorators = vec![
        (0, Decorator::Trace(0)),      // Should remain at position 0
        (2, Decorator::Trace(1)),      // Should be adjusted to position 3 due to padding
        (4, Decorator::Trace(2)),      // Should be adjusted to position 5 due to padding
    ];

    let block = BasicBlockNode::new_with_raw_decorators(
        operations,
        decorators,
        &mut mast_forest,
    ).unwrap();

    // Check that decorators are properly adjusted
    let adjusted_decorators: Vec<_> = block.decorators().iter().collect();

    // Before padding: positions [0, 2, 4]
    // After padding with one NOOP after Push: positions should be [0, 3, 5]
    assert_eq!(adjusted_decorators.len(), 3);
    assert_eq!(adjusted_decorators[0].0, 0);  // No adjustment needed
    assert_eq!(adjusted_decorators[1].0, 3);  // Adjusted by 1 due to padding
    assert_eq!(adjusted_decorators[2].0, 5);  // Adjusted by 1 due to padding

    // Verify raw operations iteration matches original unpadded sequence
    let raw_ops: Vec<_> = block.raw_operations().collect();
    assert_eq!(raw_ops.len(), 4);  // 5 original ops - 1 NOOP

    // The raw operations should be: Add, Mul, Drop (Push ops are encoded differently in batch)
    let expected_ops = vec![
        Operation::Add,
        Operation::Mul,
        Operation::Push(crate::ONE),
        Operation::Drop,
    ];

    assert_eq!(raw_ops, expected_ops.iter().collect::<Vec<_>>());
}

#[test]
fn serialization_preserves_operation_structure() {
    // Test that serialization correctly preserves the structure of operations with padding

    let operations = vec![
        Operation::Push(crate::ONE),
        Operation::Push(crate::Felt::new(2)),
        Operation::Push(crate::Felt::new(3)),
        Operation::Push(crate::Felt::new(4)),
        Operation::Push(crate::Felt::new(5)),
        Operation::Push(crate::Felt::new(6)),
        Operation::Push(crate::Felt::new(7)),
        Operation::Push(crate::Felt::new(8)),
        Operation::Push(crate::Felt::new(9)),  // This will cause overflow to next batch
    ];

    let mut mast_forest = MastForest::new();
    let decorators = vec![
        (0, Decorator::Trace(0)),  // Should be unchanged
        (8, Decorator::Trace(1)),  // Should be adjusted due to padding
    ];
    let decorators_clone = decorators.clone();

    let block = BasicBlockNode::new_with_raw_decorators(
        operations.clone(),
        decorators,
        &mut mast_forest,
    ).unwrap();

    // Create MastForest and serialize
    let mut forest = MastForest::new();
    let node_id = forest.add_block_with_raw_decorators(operations, decorators_clone).unwrap();
    forest.make_root(node_id);

    let serialized_bytes = forest.to_bytes();
    let mut reader = SliceReader::new(&serialized_bytes);
    let deserialized_forest = MastForest::read_from(&mut reader).unwrap();
    let deserialized_node_id = deserialized_forest.procedure_roots()[0];
    let deserialized_block = match &deserialized_forest[deserialized_node_id] {
        crate::mast::MastNode::Block(block) => block.clone(),
        _ => panic!("Expected basic block node"),
    };

    // Verify that the number of batches is the same
    assert_eq!(block.num_op_batches(), deserialized_block.num_op_batches());

    // Verify that decorators are properly aligned after deserialization
    let orig_decorators: Vec<_> = block.decorators().iter().collect();
    let deser_decorators: Vec<_> = deserialized_block.decorators().iter().collect();
    assert_eq!(orig_decorators, deser_decorators);

    // Verify operation counts match
    assert_eq!(block.num_operations(), deserialized_block.num_operations());

    // Verify raw operations match (unpadded)
    let orig_raw_ops: Vec<_> = block.raw_operations().collect();
    let deser_raw_ops: Vec<_> = deserialized_block.raw_operations().collect();
    assert_eq!(orig_raw_ops, deser_raw_ops);
}