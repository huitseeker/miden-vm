use super::build_test;
use crate::helpers::build_expected_hash;
use vm_core::StarkField;

#[test]
fn test_hash_empty() {
    // when the address range contains zero elements, the result is all zeros
    let empty_range = "
    use.std::crypto::hashes::native

    begin
        push.1000 # end address
        push.1000 # start address

        exec.native::hash_memory
    end
    ";
    build_test!(empty_range, &[]).expect_stack(&[0, 0, 0, 0]);

    // computes the hash for 8 consecutive zeros using mem_stream directly
    let two_zeros_mem_stream = "
    begin
        # mem_stream state
        push.1000 padw padw padw
        mem_stream

        # drop everything except the hash
        dropw swapw dropw movup.4 drop
    end
    ";

    #[rustfmt::skip]
    let zero_hash: Vec<u64> = build_expected_hash(&[
        0, 0, 0, 0,
        0, 0, 0, 0,
    ]).into_iter().map(|e| e.as_int()).collect();
    build_test!(two_zeros_mem_stream, &[]).expect_stack(&zero_hash);

    // checks the hash compute from 8 zero elements is the same when using hash_memory
    let two_zeros = "
    use.std::crypto::hashes::native

    begin
        push.1002 # end address
        push.1000 # start address

        exec.native::hash_memory
    end
    ";

    build_test!(two_zeros, &[]).expect_stack(&zero_hash);
}

#[test]
fn test_hash_one_element() {
    // computes the hash of a single 1 using mem_stream directly
    let one_memstream = "
    use.std::crypto::hashes::native

    begin
        # insert 1 to memory
        push.1.1000 mem_store

        # mem_stream state
        push.1000 padw padw padw
        mem_stream

        # drop everything except the hash
        dropw swapw dropw movup.4 drop
    end
    ";

    #[rustfmt::skip]
    let one_hash: Vec<u64> = build_expected_hash(&[
        1, 0, 0, 0,
        0, 0, 0, 0,
    ]).into_iter().map(|e| e.as_int()).collect();
    build_test!(one_memstream, &[]).expect_stack(&one_hash);

    // checks the hash of 1 is the same when using hash_memory
    let one_element = "
    use.std::crypto::hashes::native

    begin
        # insert 1 to memory
        push.1.1000 mem_store

        push.1002 # end address
        push.1000 # start address

        exec.native::hash_memory
    end
    ";

    build_test!(one_element, &[]).expect_stack(&one_hash);
}

#[test]
fn test_hash_even_words() {
    // checks the hash of two words
    let even_words = "
    use.std::crypto::hashes::native

    begin
        push.1.0.0.0.1000 mem_storew dropw
        push.0.1.0.0.1001 mem_storew dropw

        push.1002 # end address
        push.1000 # start address

        exec.native::hash_memory
    end
    ";

    #[rustfmt::skip]
    let even_hash: Vec<u64> = build_expected_hash(&[
        1, 0, 0, 0,
        0, 1, 0, 0,
    ]).into_iter().map(|e| e.as_int()).collect();
    build_test!(even_words, &[]).expect_stack(&even_hash);
}

#[test]
fn test_hash_odd_words() {
    // checks the hash of three words
    let odd_words = "
    use.std::crypto::hashes::native

    begin
        push.1.0.0.0.1000 mem_storew dropw
        push.0.1.0.0.1001 mem_storew dropw
        push.0.0.1.0.1002 mem_storew dropw

        push.1003 # end address
        push.1000 # start address

        exec.native::hash_memory
    end
    ";

    #[rustfmt::skip]
    let odd_hash: Vec<u64> = build_expected_hash(&[
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
    ]).into_iter().map(|e| e.as_int()).collect();
    build_test!(odd_words, &[]).expect_stack(&odd_hash);
}
