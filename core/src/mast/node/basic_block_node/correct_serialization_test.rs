use alloc::vec::Vec;
use std::println;
use crate::{
    Decorator, Operation,
    mast::{MastForest, DecoratorId},
    utils::{Serializable, Deserializable, SliceReader},
};

#[test]
fn test_correct_roundtrip_serialization() {
    println!("=== Testing correct round-trip serialization ===");

    let operations = vec![
        Operation::Add,
        Operation::Push(crate::ONE),
        Operation::Mul,
    ];

    // Create the first forest and block in the standard way (like the serialization expects)
    let mut original_forest = MastForest::new();

    // Add block with raw decorators - this will automatically add decorators to the forest
    // and create the correct decorator ID mappings
    let node_id = original_forest.add_block_with_raw_decorators(
        operations.clone(),
        vec![
            (0, Decorator::Trace(0)),
            (1, Decorator::Trace(1)),
            (3, Decorator::Trace(2)),
        ],
    ).unwrap();

    original_forest.make_root(node_id);

    println!("Original forest has {} decorators", original_forest.decorators().len());
    println!("Original forest decorators: {:?}", original_forest.decorators());

    let original_node_id = original_forest.procedure_roots()[0];
    let original_block = match &original_forest[original_node_id] {
        crate::mast::MastNode::Block(block) => block,
        _ => panic!("Expected basic block node"),
    };

    println!("Original block decorators: {:?}",
             original_block.decorators().iter().map(|(i, d)| (*i, d.as_u32())).collect::<Vec<_>>());

    // Serialize
    let serialized_bytes = original_forest.to_bytes();
    println!("Serialized to {} bytes", serialized_bytes.len());

    // Deserialize
    let mut reader = SliceReader::new(&serialized_bytes);
    let mut deserialized_forest = MastForest::read_from(&mut reader).unwrap();

    println!("Deserialized forest has {} decorators", deserialized_forest.decorators().len());
    println!("Deserialized forest decorators: {:?}", deserialized_forest.decorators());

    let deserialized_node_id = deserialized_forest.procedure_roots()[0];
    let deserialized_block = match &deserialized_forest[deserialized_node_id] {
        crate::mast::MastNode::Block(block) => block,
        _ => panic!("Expected basic block node"),
    };

    println!("Deserialized block decorators: {:?}",
             deserialized_block.decorators().iter().map(|(i, d)| (*i, d.as_u32())).collect::<Vec<_>>());

    // Verify that forest decorator counts match
    assert_eq!(original_forest.decorators().len(), deserialized_forest.decorators().len(),
               "Forest decorator counts should match");

    // Verify that block decorator counts match
    assert_eq!(original_block.decorators().len(), deserialized_block.decorators().len(),
               "Block decorator counts should match");

    // Verify that decorator lists are identical
    assert_eq!(original_block.decorators(), deserialized_block.decorators(),
               "Block decorator lists should be identical");

    // Test operations are preserved
    let orig_ops: Vec<_> = original_block.raw_operations().collect();
    let deser_ops: Vec<_> = deserialized_block.raw_operations().collect();
    assert_eq!(orig_ops, deser_ops, "Operations should be identical");

    println!("\n✅ Round-trip serialization test passed!");
}

#[test]
fn test_serialization_decorator_id_mapping() {
    println!("\n=== Testing serialization decorator ID mapping ===");

    // Test different numbers of decorators
    for num_decorators in [1, 2, 3, 5] {
        println!("\nTesting with {} decorators", num_decorators);

        let operations = vec![
            Operation::Push(crate::ONE),
            Operation::Add,
            Operation::Mul,
            Operation::Drop,
            Operation::Push(crate::Felt::new(42)),
            Operation::Swap,
            Operation::Eq,
            Operation::Not,
            Operation::And,
            Operation::Push(crate::Felt::new(100)),
            Operation::Swap,
            Operation::Eq,
            Operation::Not,
            Operation::And,
            Operation::Push(crate::Felt::new(200)),
        ];

        let mut original_forest = MastForest::new();

        // Create block with specified number of decorators
        let decorators: Vec<(usize, Decorator)> = (0..num_decorators)
            .map(|i| (i * 2, Decorator::Trace(i as u32)))  // Sequential positions to ensure sorting
            .collect();

        let node_id = original_forest.add_block_with_raw_decorators(
            operations.clone(),
            decorators,
        ).unwrap();

        original_forest.make_root(node_id);

        let original_node_id = original_forest.procedure_roots()[0];
        let original_block = match &original_forest[original_node_id] {
            crate::mast::MastNode::Block(block) => block,
            _ => panic!("Expected basic block node"),
        };

        // Serialize and deserialize
        let serialized_bytes = original_forest.to_bytes();
        let mut reader = SliceReader::new(&serialized_bytes);
        let mut deserialized_forest = MastForest::read_from(&mut reader).unwrap();

        let deserialized_node_id = deserialized_forest.procedure_roots()[0];
        let deserialized_block = match &deserialized_forest[deserialized_node_id] {
            crate::mast::MastNode::Block(block) => block,
            _ => panic!("Expected basic block node"),
        };

        // Verify that decorator counts match
        assert_eq!(
            original_forest.decorators().len(),
            deserialized_forest.decorators().len(),
            "Forest decorator counts should match for {} decorators", num_decorators
        );

        // Verify that block decorator lists are identical
        assert_eq!(
            original_block.decorators(),
            deserialized_block.decorators(),
            "Block decorator lists should be identical for {} decorators", num_decorators
        );

        // Verify that individual decorators match
        for (i, (orig_idx, orig_dec)) in original_block.decorators().iter().enumerate() {
            let (deser_idx, deser_dec) = deserialized_block.decorators()[i];

            assert_eq!(*orig_idx, deser_idx,
                      "Decorator indices should match for decorator {}", i);
            assert_eq!(*orig_dec, deser_dec,
                      "Decorator IDs should match for decorator {}", i);
        }

        println!("✅ {} decorators: OK", num_decorators);
    }

    println!("\n✅ All decorator ID mapping tests passed!");
}