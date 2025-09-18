use alloc::vec::Vec;

use miden_crypto::WORD_SIZE;
use proptest::prelude::*;
use winter_math::FieldElement;
use winter_rand_utils::prng_array;

use crate::{
    Decorator, Felt, Kernel, Operation, ProgramInfo, Word,
    chiplets::hasher,
    mast::{
        BasicBlockNode, DecoratorId, DynNode, MastForest, MastNodeExt, node::MastNodeErrorContext,
    },
    utils::{Deserializable, Serializable},
};

#[test]
fn dyn_hash_is_correct() {
    let expected_constant =
        hasher::merge_in_domain(&[Word::default(), Word::default()], DynNode::DYN_DOMAIN);
    assert_eq!(expected_constant, DynNode::new_dyn().digest());
}

proptest! {
    #[test]
    fn arbitrary_program_info_serialization_works(
        kernel_count in prop::num::u8::ANY,
        ref seed in any::<[u8; 32]>()
    ) {
        let program_hash = digest_from_seed(*seed);
        let kernel: Vec<Word> = (0..kernel_count)
            .scan(*seed, |seed, _| {
                *seed = prng_array(*seed);
                Some(digest_from_seed(*seed))
            })
            .collect();
        let kernel = Kernel::new(&kernel).unwrap();
        let program_info = ProgramInfo::new(program_hash, kernel);
        let bytes = program_info.to_bytes();
        let deser = ProgramInfo::read_from_bytes(&bytes).unwrap();
        assert_eq!(program_info, deser);
    }
}

#[test]
fn test_new_decorator_pattern() {
    let mut forest = MastForest::new();

    // Create decorators
    let deco1 = forest.add_decorator(Decorator::Trace(1)).unwrap();
    let deco2 = forest.add_decorator(Decorator::Trace(2)).unwrap();

    // Test the new pattern
    let operations =
        vec![Operation::Push(Felt::new(1)), Operation::Add, Operation::Push(Felt::new(2))];

    let decorators = vec![
        (0, deco1), // Decorator at operation index 0
        (2, deco2), // Decorator at operation index 2
    ];

    // Use the new add_block method
    let block_id = forest.add_block(operations.clone(), decorators.clone()).unwrap();

    // Verify the block was created
    assert!(forest.nodes.get(block_id.as_usize()).is_some());

    // Verify that the block_decorators field is populated
    assert!(forest.block_decorators.contains_key(&block_id));
    let stored_decorators = &forest.block_decorators[&block_id];
    assert_eq!(stored_decorators.len(), 2);

    // Verify the block has the same decorators as what was provided (adjusted for op_batches)
    let block = if let crate::mast::MastNode::Block(block) = &forest[block_id] {
        block
    } else {
        panic!("Expected a block node");
    };

    let block_decorators: Vec<_> = MastNodeErrorContext::decorators(block).collect();
    assert_eq!(block_decorators.len(), 2);

    // Verify that the adjust_decorators method works correctly
    let adjusted_decorators =
        BasicBlockNode::adjust_decorators(decorators.clone(), block.op_batches());
    let expected_adjusted: Vec<_> =
        adjusted_decorators.iter().map(|(idx, id)| (*idx, *id)).collect();
    let stored_adjusted: Vec<_> = stored_decorators.iter().map(|(idx, id)| (*idx, *id)).collect();

    assert_eq!(stored_adjusted, expected_adjusted);
}

#[test]
fn test_block_decorators_storage() {
    let mut forest = MastForest::new();

    // Test adding a block with decorators
    let operations = vec![
        crate::Operation::Push(Felt::new(1)),
        crate::Operation::Add,
        crate::Operation::Push(Felt::new(2)),
    ];

    // Add decorator to forest and get its ID
    let decorator_id = forest.add_decorator(Decorator::Trace(42)).unwrap();
    let decorators = vec![(0, decorator_id)];
    let block_id = forest.add_block(operations.clone(), decorators).unwrap();

    // Verify that block_decorators contains the decorators
    assert!(forest.block_decorators.contains_key(&block_id));

    let stored_decorators = &forest.block_decorators[&block_id];
    assert_eq!(stored_decorators.len(), 1);
    assert_eq!(stored_decorators[0].0, 0); // operation index
    assert_eq!(stored_decorators[0].1, DecoratorId(0)); // decorator id

    // Test adding another block without decorators
    let operations2 = vec![crate::Operation::Push(Felt::new(3))];
    let block_id2 = forest.add_block(operations2.clone(), vec![]).unwrap();

    // Verify that empty decorators are not stored
    assert!(!forest.block_decorators.contains_key(&block_id2));

    // Test strip_decorators clears block_decorators
    forest.strip_decorators();
    assert!(forest.block_decorators.is_empty());
}

// HELPER FUNCTIONS
// --------------------------------------------------------------------------------------------

fn digest_from_seed(seed: [u8; 32]) -> Word {
    let mut digest = [Felt::ZERO; WORD_SIZE];
    digest.iter_mut().enumerate().for_each(|(i, d)| {
        *d = <[u8; 8]>::try_from(&seed[i * 8..(i + 1) * 8])
            .map(u64::from_le_bytes)
            .map(Felt::new)
            .unwrap()
    });
    digest.into()
}
