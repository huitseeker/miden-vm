use alloc::string::ToString;

use miden_crypto::{Felt, ONE, Word};

use super::*;
use crate::{AssemblyOp, DebugOptions, Decorator, mast::MastForestError, operations::Operation};

/// If this test fails to compile, it means that `Operation` or `Decorator` was changed. Make sure
/// that all tests in this file are updated accordingly. For example, if a new `Operation` variant
/// was added, make sure that you add it in the vector of operations in
/// [`serialize_deserialize_all_nodes`].
#[test]
fn confirm_operation_and_decorator_structure() {
    match Operation::Noop {
        Operation::Noop => (),
        Operation::Assert(_) => (),
        Operation::FmpAdd => (),
        Operation::FmpUpdate => (),
        Operation::SDepth => (),
        Operation::Caller => (),
        Operation::Clk => (),
        Operation::Join => (),
        Operation::Split => (),
        Operation::Loop => (),
        Operation::Call => (),
        Operation::Dyn => (),
        Operation::Dyncall => (),
        Operation::SysCall => (),
        Operation::Span => (),
        Operation::End => (),
        Operation::Repeat => (),
        Operation::Respan => (),
        Operation::Halt => (),
        Operation::Add => (),
        Operation::Neg => (),
        Operation::Mul => (),
        Operation::Inv => (),
        Operation::Incr => (),
        Operation::And => (),
        Operation::Or => (),
        Operation::Not => (),
        Operation::Eq => (),
        Operation::Eqz => (),
        Operation::Expacc => (),
        Operation::Ext2Mul => (),
        Operation::U32split => (),
        Operation::U32add => (),
        Operation::U32assert2(_) => (),
        Operation::U32add3 => (),
        Operation::U32sub => (),
        Operation::U32mul => (),
        Operation::U32madd => (),
        Operation::U32div => (),
        Operation::U32and => (),
        Operation::U32xor => (),
        Operation::Pad => (),
        Operation::Drop => (),
        Operation::Dup0 => (),
        Operation::Dup1 => (),
        Operation::Dup2 => (),
        Operation::Dup3 => (),
        Operation::Dup4 => (),
        Operation::Dup5 => (),
        Operation::Dup6 => (),
        Operation::Dup7 => (),
        Operation::Dup9 => (),
        Operation::Dup11 => (),
        Operation::Dup13 => (),
        Operation::Dup15 => (),
        Operation::Swap => (),
        Operation::SwapW => (),
        Operation::SwapW2 => (),
        Operation::SwapW3 => (),
        Operation::SwapDW => (),
        Operation::MovUp2 => (),
        Operation::MovUp3 => (),
        Operation::MovUp4 => (),
        Operation::MovUp5 => (),
        Operation::MovUp6 => (),
        Operation::MovUp7 => (),
        Operation::MovUp8 => (),
        Operation::MovDn2 => (),
        Operation::MovDn3 => (),
        Operation::MovDn4 => (),
        Operation::MovDn5 => (),
        Operation::MovDn6 => (),
        Operation::MovDn7 => (),
        Operation::MovDn8 => (),
        Operation::CSwap => (),
        Operation::CSwapW => (),
        Operation::Push(_) => (),
        Operation::AdvPop => (),
        Operation::AdvPopW => (),
        Operation::MLoadW => (),
        Operation::MStoreW => (),
        Operation::MLoad => (),
        Operation::MStore => (),
        Operation::MStream => (),
        Operation::Pipe => (),
        Operation::HPerm => (),
        Operation::MpVerify(_) => (),
        Operation::MrUpdate => (),
        Operation::FriE2F4 => (),
        Operation::HornerBase => (),
        Operation::HornerExt => (),
        Operation::EvalCircuit => (),
        Operation::Emit => (),
    };

    match Decorator::Trace(0) {
        Decorator::AsmOp(_) => (),
        Decorator::Debug(debug_options) => match debug_options {
            DebugOptions::StackAll => (),
            DebugOptions::StackTop(_) => (),
            DebugOptions::MemAll => (),
            DebugOptions::MemInterval(..) => (),
            DebugOptions::LocalInterval(..) => (),
            DebugOptions::AdvStackTop(_) => (),
        },
        Decorator::Trace(_) => (),
    };
}

#[test]
fn serialize_deserialize_all_nodes() {
    let mut mast_forest = MastForest::new();

    let basic_block_id = {
        let operations = vec![
            Operation::Noop,
            Operation::Assert(Felt::from(42u32)),
            Operation::FmpAdd,
            Operation::FmpUpdate,
            Operation::SDepth,
            Operation::Caller,
            Operation::Clk,
            Operation::Join,
            Operation::Split,
            Operation::Loop,
            Operation::Call,
            Operation::Dyn,
            Operation::SysCall,
            Operation::Span,
            Operation::End,
            Operation::Repeat,
            Operation::Respan,
            Operation::Halt,
            Operation::Add,
            Operation::Neg,
            Operation::Mul,
            Operation::Inv,
            Operation::Incr,
            Operation::And,
            Operation::Or,
            Operation::Not,
            Operation::Eq,
            Operation::Eqz,
            Operation::Expacc,
            Operation::Ext2Mul,
            Operation::U32split,
            Operation::U32add,
            Operation::U32assert2(Felt::from(222u32)),
            Operation::U32add3,
            Operation::U32sub,
            Operation::U32mul,
            Operation::U32madd,
            Operation::U32div,
            Operation::U32and,
            Operation::U32xor,
            Operation::Pad,
            Operation::Drop,
            Operation::Dup0,
            Operation::Dup1,
            Operation::Dup2,
            Operation::Dup3,
            Operation::Dup4,
            Operation::Dup5,
            Operation::Dup6,
            Operation::Dup7,
            Operation::Dup9,
            Operation::Dup11,
            Operation::Dup13,
            Operation::Dup15,
            Operation::Swap,
            Operation::SwapW,
            Operation::SwapW2,
            Operation::SwapW3,
            Operation::SwapDW,
            Operation::MovUp2,
            Operation::MovUp3,
            Operation::MovUp4,
            Operation::MovUp5,
            Operation::MovUp6,
            Operation::MovUp7,
            Operation::MovUp8,
            Operation::MovDn2,
            Operation::MovDn3,
            Operation::MovDn4,
            Operation::MovDn5,
            Operation::MovDn6,
            Operation::MovDn7,
            Operation::MovDn8,
            Operation::CSwap,
            Operation::CSwapW,
            Operation::Push(Felt::new(45)),
            Operation::AdvPop,
            Operation::AdvPopW,
            Operation::MLoadW,
            Operation::MStoreW,
            Operation::MLoad,
            Operation::MStore,
            Operation::MStream,
            Operation::Pipe,
            Operation::HPerm,
            Operation::MpVerify(Felt::from(1022u32)),
            Operation::MrUpdate,
            Operation::FriE2F4,
            Operation::HornerBase,
            Operation::HornerExt,
            Operation::Emit,
        ];

        let num_operations = operations.len();

        let decorators = vec![
            (
                0,
                Decorator::AsmOp(AssemblyOp::new(
                    Some(miden_debug_types::Location {
                        uri: "test".into(),
                        start: 42.into(),
                        end: 43.into(),
                    }),
                    "context".to_string(),
                    15,
                    "op".to_string(),
                    false,
                )),
            ),
            (0, Decorator::Debug(DebugOptions::StackAll)),
            (15, Decorator::Debug(DebugOptions::StackTop(255))),
            (15, Decorator::Debug(DebugOptions::MemAll)),
            (15, Decorator::Debug(DebugOptions::MemInterval(0, 16))),
            (17, Decorator::Debug(DebugOptions::LocalInterval(1, 2, 3))),
            (19, Decorator::Debug(DebugOptions::AdvStackTop(255))),
            (num_operations, Decorator::Trace(55)),
        ];

        mast_forest.add_block_with_raw_decorators(operations, decorators).unwrap()
    };

    // Decorators to add to following nodes
    let decorator_id1 = mast_forest.add_decorator(Decorator::Trace(1)).unwrap();
    let decorator_id2 = mast_forest.add_decorator(Decorator::Trace(2)).unwrap();

    // Call node
    let call_node_id = mast_forest.add_call(basic_block_id).unwrap();
    mast_forest[call_node_id].append_before_enter(&[decorator_id1]);
    mast_forest[call_node_id].append_after_exit(&[decorator_id2]);

    // Syscall node
    let syscall_node_id = mast_forest.add_syscall(basic_block_id).unwrap();
    mast_forest[syscall_node_id].append_before_enter(&[decorator_id1]);
    mast_forest[syscall_node_id].append_after_exit(&[decorator_id2]);

    // Loop node
    let loop_node_id = mast_forest.add_loop(basic_block_id).unwrap();
    mast_forest[loop_node_id].append_before_enter(&[decorator_id1]);
    mast_forest[loop_node_id].append_after_exit(&[decorator_id2]);

    // Join node
    let join_node_id = mast_forest.add_join(basic_block_id, call_node_id).unwrap();
    mast_forest[join_node_id].append_before_enter(&[decorator_id1]);
    mast_forest[join_node_id].append_after_exit(&[decorator_id2]);

    // Split node
    let split_node_id = mast_forest.add_split(basic_block_id, call_node_id).unwrap();
    mast_forest[split_node_id].append_before_enter(&[decorator_id1]);
    mast_forest[split_node_id].append_after_exit(&[decorator_id2]);

    // Dyn node
    let dyn_node_id = mast_forest.add_dyn().unwrap();
    mast_forest[dyn_node_id].append_before_enter(&[decorator_id1]);
    mast_forest[dyn_node_id].append_after_exit(&[decorator_id2]);

    // Dyncall node
    let dyncall_node_id = mast_forest.add_dyncall().unwrap();
    mast_forest[dyncall_node_id].append_before_enter(&[decorator_id1]);
    mast_forest[dyncall_node_id].append_after_exit(&[decorator_id2]);

    // External node
    let external_node_id = mast_forest.add_external(Word::default()).unwrap();
    mast_forest[external_node_id].append_before_enter(&[decorator_id1]);
    mast_forest[external_node_id].append_after_exit(&[decorator_id2]);

    mast_forest.make_root(join_node_id);
    mast_forest.make_root(syscall_node_id);
    mast_forest.make_root(loop_node_id);
    mast_forest.make_root(split_node_id);
    mast_forest.make_root(dyn_node_id);
    mast_forest.make_root(dyncall_node_id);
    mast_forest.make_root(external_node_id);

    let serialized_mast_forest = mast_forest.to_bytes();
    let deserialized_mast_forest = MastForest::read_from_bytes(&serialized_mast_forest).unwrap();

    assert_eq!(mast_forest, deserialized_mast_forest);
}

/// Test that a forest with a node whose child ids are larger than its own id serializes and
/// deserializes successfully.
#[test]
fn mast_forest_serialize_deserialize_with_child_ids_exceeding_parent_id() {
    let mut forest = MastForest::new();
    let deco0 = forest.add_decorator(Decorator::Trace(0)).unwrap();
    let deco1 = forest.add_decorator(Decorator::Trace(1)).unwrap();
    let zero = forest.add_block(vec![Operation::U32div], None).unwrap();
    let first = forest.add_block(vec![Operation::U32add], Some(vec![(0, deco0)])).unwrap();
    let second = forest.add_block(vec![Operation::U32and], Some(vec![(1, deco1)])).unwrap();
    forest.add_join(first, second).unwrap();

    // Move the Join node before its child nodes and remove the temporary zero node.
    forest.nodes.swap_remove(zero.as_usize());

    MastForest::read_from_bytes(&forest.to_bytes()).unwrap();
}

/// Test that a forest with a node whose referenced index is >= the max number of nodes in
/// the forest returns an error during deserialization.
#[test]
fn mast_forest_serialize_deserialize_with_overflowing_ids_fails() {
    let mut overflow_forest = MastForest::new();
    let id0 = overflow_forest.add_block(vec![Operation::Eqz], None).unwrap();
    overflow_forest.add_block(vec![Operation::Eqz], None).unwrap();
    let id2 = overflow_forest.add_block(vec![Operation::Eqz], None).unwrap();
    let id_join = overflow_forest.add_join(id0, id2).unwrap();

    let join_node = overflow_forest[id_join].clone();

    // Add the Join(0, 2) to this forest which does not have a node with index 2.
    let mut forest = MastForest::new();
    let deco0 = forest.add_decorator(Decorator::Trace(0)).unwrap();
    let deco1 = forest.add_decorator(Decorator::Trace(1)).unwrap();
    forest
        .add_block(vec![Operation::U32add], Some(vec![(0, deco0), (1, deco1)]))
        .unwrap();
    forest.add_node(join_node).unwrap();

    assert_matches!(
        MastForest::read_from_bytes(&forest.to_bytes()),
        Err(DeserializationError::InvalidValue(msg)) if msg.contains("number of nodes")
    );
}

#[test]
fn mast_forest_invalid_node_id() {
    // Hydrate a forest smaller than the second
    let mut forest = MastForest::new();
    let first = forest.add_block(vec![Operation::U32div], None).unwrap();
    let second = forest.add_block(vec![Operation::U32div], None).unwrap();

    // Hydrate a forest larger than the first to get an overflow MastNodeId
    let mut overflow_forest = MastForest::new();

    overflow_forest.add_block(vec![Operation::U32div], None).unwrap();
    overflow_forest.add_block(vec![Operation::U32div], None).unwrap();
    overflow_forest.add_block(vec![Operation::U32div], None).unwrap();
    let overflow = overflow_forest.add_block(vec![Operation::U32div], None).unwrap();

    // Attempt to join with invalid ids
    let join = forest.add_join(overflow, second);
    assert_eq!(join, Err(MastForestError::NodeIdOverflow(overflow, 2)));
    let join = forest.add_join(first, overflow);
    assert_eq!(join, Err(MastForestError::NodeIdOverflow(overflow, 2)));

    // Attempt to split with invalid ids
    let split = forest.add_split(overflow, second);
    assert_eq!(split, Err(MastForestError::NodeIdOverflow(overflow, 2)));
    let split = forest.add_split(first, overflow);
    assert_eq!(split, Err(MastForestError::NodeIdOverflow(overflow, 2)));

    // Attempt to loop with invalid ids
    assert_eq!(forest.add_loop(overflow), Err(MastForestError::NodeIdOverflow(overflow, 2)));

    // Attempt to call with invalid ids
    assert_eq!(forest.add_call(overflow), Err(MastForestError::NodeIdOverflow(overflow, 2)));
    assert_eq!(forest.add_syscall(overflow), Err(MastForestError::NodeIdOverflow(overflow, 2)));

    // Validate normal operations
    forest.add_join(first, second).unwrap();
}

/// Test `MastForest::advice_map` serialization and deserialization.
#[test]
fn mast_forest_serialize_deserialize_advice_map() {
    let mut forest = MastForest::new();
    let deco0 = forest.add_decorator(Decorator::Trace(0)).unwrap();
    let deco1 = forest.add_decorator(Decorator::Trace(1)).unwrap();
    let first = forest.add_block(vec![Operation::U32add], Some(vec![(0, deco0)])).unwrap();
    let second = forest.add_block(vec![Operation::U32and], Some(vec![(1, deco1)])).unwrap();
    forest.add_join(first, second).unwrap();

    let key = Word::new([ONE, ONE, ONE, ONE]);
    let value = vec![ONE, ONE];

    forest.advice_map_mut().insert(key, value);

    let parsed = MastForest::read_from_bytes(&forest.to_bytes()).unwrap();
    assert_eq!(forest.advice_map, parsed.advice_map);
}

// ================================================================================
// Decorator Offset Serialization Tests
// ================================================================================

/// Test that demonstrates decorator offset calculation issues during operation padding.
///
/// This test reveals a bug where decorator offsets are not properly adjusted when operations
/// are padded during the batch_ops process. The decorator that was originally at index 3
/// ends up at the wrong position after serialization/deserialization.
#[test]
fn decorator_offset_mismatch_after_padding() {
    // Operations that will trigger padding (GROUP_SIZE = 9, so 17 operations = 2 batches)
    let operations = vec![
        Operation::Add,
        Operation::Push(ONE),
        Operation::Mul,
        Operation::Push(Felt::new(42)),    // Decorator at index 3
        Operation::Drop,
        Operation::Swap,
        Operation::Eq,
        Operation::Not,
        Operation::And,
        Operation::Push(Felt::new(100)),
        Operation::Swap,
        Operation::Eq,
        Operation::Not,
        Operation::And,
        Operation::Push(Felt::new(200)),
        Operation::Swap,
        Operation::Eq,
    ];

    let mut original_forest = MastForest::new();

    // Add decorator at operation index 3 (before padding)
    let decorators = vec![(3, Decorator::Trace(123))];
    let node_id = original_forest.add_block_with_raw_decorators(operations, decorators).unwrap();
    original_forest.make_root(node_id);

    // Serialize and deserialize
    let serialized_bytes = original_forest.to_bytes();
    let mut reader = crate::utils::SliceReader::new(&serialized_bytes);
    let deserialized_forest = MastForest::read_from(&mut reader).unwrap();

    let deserialized_node_id = deserialized_forest.procedure_roots()[0];
    let original_block = &original_forest[node_id];
    let deserialized_block = &deserialized_forest[deserialized_node_id];

    // Both blocks should be basic blocks for this test
    let original_block = match original_block {
        crate::mast::MastNode::Block(block) => block,
        _ => panic!("Expected basic block node"),
    };
    let deserialized_block = match deserialized_block {
        crate::mast::MastNode::Block(block) => block,
        _ => panic!("Expected basic block node"),
    };

    // The decorator index should be preserved after round-trip serialization
    // This assertion will fail due to the decorator offset bug
    assert_eq!(
        original_block.decorators(),
        deserialized_block.decorators(),
        "Decorator indices should match after round-trip serialization"
    );
}

/// Test that demonstrates invalid decorator ID references after deserialization.
///
/// This test reveals a bug where deserialization creates invalid decorator ID references
/// that point to decorators that don't exist in the deserialized forest.
#[test]
fn invalid_decorator_id_references_after_deserialization() {
    // Create operations that will be padded during batch processing
    let operations = vec![
        Operation::Push(ONE),
        Operation::Add,
        Operation::Mul,
        Operation::Drop,
        Operation::Push(Felt::new(42)),
        Operation::Swap,
        Operation::Eq,
        Operation::Not,
        Operation::And,    // Index 8 (last in first batch)
        Operation::Push(Felt::new(100)),  // Index 9 (first in second batch)
    ];

    let mut original_forest = MastForest::new();

    // Add decorators at positions that will be affected by padding
    let decorators = vec![
        (0, Decorator::Trace(0)),
        (1, Decorator::Trace(1)),
        (9, Decorator::Trace(2)),  // This decorator will be affected by padding
    ];
    let node_id = original_forest.add_block_with_raw_decorators(operations, decorators).unwrap();
    original_forest.make_root(node_id);

    // Check decorator count before serialization
    let original_decorator_count = original_forest.decorators().len();
    assert_eq!(original_decorator_count, 3, "Should have 3 decorators before serialization");

    // Serialize and deserialize
    let serialized_bytes = original_forest.to_bytes();
    let mut reader = crate::utils::SliceReader::new(&serialized_bytes);
    let deserialized_forest = MastForest::read_from(&mut reader).unwrap();

    // Check decorator count after deserialization
    let deserialized_decorator_count = deserialized_forest.decorators().len();
    assert_eq!(
        deserialized_decorator_count, original_decorator_count,
        "Decorator count should match after deserialization"
    );

    let deserialized_node_id = deserialized_forest.procedure_roots()[0];
    let deserialized_block = match &deserialized_forest[deserialized_node_id] {
        crate::mast::MastNode::Block(block) => block,
        _ => panic!("Expected basic block node"),
    };

    // All decorator IDs should be valid references within the deserialized forest
    for (_, decorator_id) in deserialized_block.decorators().iter() {
        let decorator_index = decorator_id.as_usize();
        assert!(
            decorator_index < deserialized_decorator_count,
            "Invalid decorator ID {}: only {} decorators exist in forest",
            decorator_index,
            deserialized_decorator_count
        );
    }

    // The decorator lists should be identical after round-trip serialization
    let original_block = match &original_forest[node_id] {
        crate::mast::MastNode::Block(block) => block,
        _ => panic!("Expected basic block node"),
    };

    assert_eq!(
        original_block.decorators(),
        deserialized_block.decorators(),
        "Decorator lists should be identical after round-trip serialization"
    );
}

/// Test that demonstrates decorator padding offset calculation bug.
///
/// This test reveals a specific bug where decorator offset calculation during operation
/// padding is incorrect. Expected adjustment is 3, but actual adjustment is 2.
#[test]
fn decorator_padding_offset_calculation_bug() {
    // Operations that will cause padding in the middle
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

    let block = crate::mast::BasicBlockNode::new_with_raw_decorators(
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
}

/// Test that demonstrates invalid MAST decorator ID after serialization/deserialization.
///
/// This test reveals a bug where deserialization creates invalid decorator ID references
/// that point to decorators that don't exist in the deserialized forest.
#[test]
fn invalid_mast_decorator_id_after_deserialization() {
    // Test case from roundtrip_test.rs that was failing
    let operations = vec![
        Operation::Add,
        Operation::Push(crate::ONE),
        Operation::Mul,
        Operation::Push(crate::Felt::new(42)),
        Operation::Drop,
        Operation::MovDn2,
        Operation::Pad,
        Operation::Swap,
        Operation::Dup1,
    ];

    let mut mast_forest = MastForest::new();
    let decorators = vec![
        (0, Decorator::Trace(0)),      // Before first operation (should be adjusted for padding)
        (1, Decorator::Trace(1)),      // After Push(ONE)
        (4, Decorator::Trace(2)),      // After Drop (affected by padding)
        (9, Decorator::Trace(3)),      // After last operation
    ];

    // Create the original BasicBlockNode
    let original_block = crate::mast::BasicBlockNode::new_with_raw_decorators(
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
    let mut reader = crate::utils::SliceReader::new(&serialized_bytes);
    let deserialized_forest = MastForest::read_from(&mut reader).unwrap();
    let deserialized_node_id = deserialized_forest.procedure_roots()[0];
    let deserialized_block = match &deserialized_forest[deserialized_node_id] {
        crate::mast::MastNode::Block(block) => block.clone(),
        _ => panic!("Expected basic block node"),
    };

    // The decorator lists should be identical after round-trip serialization
    // This assertion will fail due to the decorator ID mapping bug
    assert_eq!(
        original_block.decorators(),
        deserialized_block.decorators(),
        "Decorator lists should be identical after round-trip serialization"
    );
}
