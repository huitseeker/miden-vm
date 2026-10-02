//! Reproduction for: recursive-group canonical order is ambiguous for tied declared
//! names, so valid groups fail round-trip with "definitions are not in canonical order".
//!
//! Issue: https://github.com/0xMiden/miden-vm/issues/3963

use miden_serde_utils::{Deserializable, Serializable, SliceReader};
use midenc_hir_type::{RecursiveTypeBuilder, StructTemplate, Type, TypeRepr, TypeTemplate};

/// A group where the tied declared names (all anonymous structs) make the canonical rank
/// depend on blind-key renderings that differ between the builder and the decoder:
/// `B` references the COMPLETED out-of-group definition `C`, and the members reference
/// each other through `List` barriers. The wire form fails canonical-order validation
/// on decode, i.e. a validly-constructed group cannot round-trip.
#[test]
fn group_with_completed_reference_fails_canonical_order_decoding() {
    let mut builder = RecursiveTypeBuilder::new();
    builder
        .define_struct(
            "A",
            StructTemplate::new(
                TypeRepr::Default,
                [
                    ("f0", TypeTemplate::ptr(TypeTemplate::Type(Type::Felt))),
                    ("f1", TypeTemplate::ptr(TypeTemplate::rec("A"))),
                    ("f2", TypeTemplate::list(TypeTemplate::rec("B"))),
                ],
            ),
        )
        .define_struct(
            "B",
            StructTemplate::new(
                TypeRepr::Default,
                [
                    ("f0", TypeTemplate::ptr(TypeTemplate::rec("C"))),
                    ("f1", TypeTemplate::list(TypeTemplate::rec("A"))),
                    ("f2", TypeTemplate::Type(Type::U8)),
                ],
            ),
        )
        .define_struct(
            "C",
            StructTemplate::new(TypeRepr::Default, [("f0", TypeTemplate::Type(Type::U128))]),
        );
    let built = builder.build().expect("group should build");

    // Both group members fail the same way; C (outside the group) round-trips.
    for key in ["A", "B"] {
        let ty = built.get(key).expect(key).clone();
        let mut bytes = Vec::new();
        ty.write_into(&mut bytes);
        let err = Type::read_from(&mut SliceReader::new(&bytes))
            .expect_err("a valid group fails to decode");
        match err {
            miden_serde_utils::DeserializationError::InvalidValue(message) => {
                assert_eq!(
                    message, "invalid recursive type: definitions are not in canonical order",
                    "expected the canonical-order rejection for {key}",
                );
            },
            other => panic!("expected InvalidValue for {key}, got {other:?}"),
        }
    }

    let c = built.get("C").expect("C").clone();
    let mut bytes = Vec::new();
    c.write_into(&mut bytes);
    let decoded = Type::read_from(&mut SliceReader::new(&bytes)).expect("C should decode");
    assert_eq!(decoded, c);
}
