use alloc::vec::Vec;
use std::println;
use crate::{
    Decorator, Operation,
    mast::{MastForest, BasicBlockNode, DecoratorId},
    utils::{Serializable, Deserializable, SliceReader},
};

#[test]
fn test_decorator_id_mapping_issue() {
    println!("=== Testing decorator ID mapping issue ===");

    let operations = vec![
        Operation::Add,
        Operation::Push(crate::ONE),
        Operation::Mul,
    ];

    // Create first forest and add some decorators
    let mut forest1 = MastForest::new();

    // Add 3 decorators to forest1
    let trace1 = forest1.add_decorator(Decorator::Trace(0)).unwrap();
    let trace2 = forest1.add_decorator(Decorator::Trace(1)).unwrap();
    let trace3 = forest1.add_decorator(Decorator::Trace(2)).unwrap();

    println!("Forest1 has {} decorators: {:?}", forest1.decorators().len(),
             (0..forest1.decorators().len()).collect::<Vec<_>>());

    // Create block with decorators in forest1
    let decorators_for_forest1 = vec![
        (0, Decorator::Trace(0)),    // This gets converted to DecoratorId internally
        (1, Decorator::Trace(1)),
        (3, Decorator::Trace(2)),
    ];

    let node_id1 = forest1.add_block_with_raw_decorators(
        operations.clone(),
        decorators_for_forest1,
    ).unwrap();

    forest1.make_root(node_id1);

    // Serialize forest1
    let serialized_bytes = forest1.to_bytes();
    println!("Forest1 serialized to {} bytes", serialized_bytes.len());

    // Deserialize to forest2
    let mut reader = SliceReader::new(&serialized_bytes);
    let mut forest2 = MastForest::read_from(&mut reader).unwrap();

    println!("Forest2 has {} decorators: {:?}", forest2.decorators().len(),
             (0..forest2.decorators().len()).collect::<Vec<_>>());

    // Check that we have the expected number of decorators
    assert_eq!(forest1.decorators().len(), forest2.decorators().len(),
               "Forest decorator counts should match");

    // Get the nodes
    let original_node_id = forest1.procedure_roots()[0];
    let deserialized_node_id = forest2.procedure_roots()[0];

    let original_block = match &forest1[original_node_id] {
        crate::mast::MastNode::Block(block) => block.clone(),
        _ => panic!("Expected basic block node"),
    };

    let deserialized_block = match &forest2[deserialized_node_id] {
        crate::mast::MastNode::Block(block) => block.clone(),
        _ => panic!("Expected basic block node"),
    };

    println!("\n=== Block Decorator Comparison ===");
    println!("Original block decorators: {:?}",
             original_block.decorators().iter().map(|(i, d)| (i, d.as_u32())).collect::<Vec<_>>());
    println!("Deserialized block decorators: {:?}",
             deserialized_block.decorators().iter().map(|(i, d)| (i, d.as_u32())).collect::<Vec<_>>());

    // Check decorator counts
    assert_eq!(original_block.decorators().len(), deserialized_block.decorators().len(),
               "Block decorator counts should match");

    // Check each decorator's index
    for (i, orig_decorator) in original_block.decorators().iter().enumerate() {
        let deser_decorator = &deserialized_block.decorators()[i];

        println!("Decorator {}: original idx={}, deserialized idx={}, orig_dec_id={}, deser_dec_id={}",
                 i, orig_decorator.0, deser_decorator.0, orig_decorator.1.as_u32(), deser_decorator.1.as_u32());

        // The indices should be identical (position in operation stream)
        assert_eq!(orig_decorator.0, deser_decorator.0,
                   "Decorator indices should be preserved");
    }
}

#[test]
fn test_forest_serialization_details() {
    println!("\n=== Testing forest serialization details ===");

    // Create a simple block
    let operations = vec![
        Operation::Push(crate::ONE),
        Operation::Add,
    ];

    let mut forest1 = MastForest::new();

    // Add 2 decorators to forest1
    let dec1 = forest1.add_decorator(Decorator::Trace(42)).unwrap();
    let dec2 = forest1.add_decorator(Decorator::Trace(43)).unwrap();

    println!("Added decorators to forest1: dec1={}, dec2={}", dec1.as_u32(), dec2.as_u32());

    // Create block with decorators
    let block_decorators = vec![
        (0, Decorator::Trace(42)),
        (1, Decorator::Trace(43)),
    ];

    let node_id1 = forest1.add_block_with_raw_decorators(
        operations.clone(),
        block_decorators,
    ).unwrap();

    forest1.make_root(node_id1);

    println!("Forest1 before serialization:");
    println!("  - {} decorators", forest1.decorators().len());
    println!("  - Root node: {:?}", node_id1);

    // Serialize forest1
    let serialized_bytes = forest1.to_bytes();
    println!("Serialized to {} bytes", serialized_bytes.len());

    // Deserialize to forest2
    let mut reader = SliceReader::new(&serialized_bytes);
    let mut forest2 = MastForest::read_from(&mut reader).unwrap();

    println!("Forest2 after deserialization:");
    println!("  - {} decorators", forest2.decorators().len());
    println!("  - Root node: {:?}", forest2.procedure_roots()[0]);

    // Check decorator mapping
    println!("\nForest1 has {} decorators", forest1.decorators().len());
    println!("Forest2 has {} decorators", forest2.decorators().len());

    // Verify we have the same number of decorators
    assert_eq!(forest1.decorators().len(), forest2.decorators().len(),
               "Forest decorator counts should match");

    // Get the blocks and compare their decorators
    let original_node_id = forest1.procedure_roots()[0];
    let deserialized_node_id = forest2.procedure_roots()[0];

    let original_block = match &forest1[original_node_id] {
        crate::mast::MastNode::Block(block) => block,
        _ => panic!("Expected basic block node"),
    };

    let deserialized_block = match &forest2[deserialized_node_id] {
        crate::mast::MastNode::Block(block) => block,
        _ => panic!("Expected basic block node"),
    };

    println!("\nOriginal block decorators: {:?}",
             original_block.decorators().iter().map(|(i, d)| (*i, d.as_u32())).collect::<Vec<_>>());
    println!("Deserialized block decorators: {:?}",
             deserialized_block.decorators().iter().map(|(i, d)| (*i, d.as_u32())).collect::<Vec<_>>());

    // The decorator lists should be identical
    assert_eq!(original_block.decorators(), deserialized_block.decorators(),
               "Decorator lists should be identical");
}