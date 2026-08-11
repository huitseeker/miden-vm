use alloc::{sync::Arc, vec::Vec};

use super::{
    DeferredState, DeferredStateWire, Digest, IntegrityError, MAX_PRECOMPILE_ROOTS,
    PrecompileError, PrecompileRegistry, TRUE_DIGEST, fold_deferred_root,
};
use crate::serde::{
    BudgetedReader, ByteReader, ByteWriter, Deserializable, DeserializationError, Serializable,
    SliceReader,
};

/// A hydrated witness for one or more ordered deferred precompile roots.
///
/// Execution constructs singleton witnesses. [`Self::merge`] combines only singleton inputs and
/// retains their roots in input order, including duplicates.
///
/// A witness may contain private execution data and a large hydrated DAG. Treat it as sensitive
/// prover input, and prefer borrowing it during proving instead of cloning it.
#[derive(Debug, Clone)]
pub struct PrecompileWitness {
    state: DeferredState,
    roots: Vec<Digest>,
}

impl PrecompileWitness {
    /// Creates a singleton witness from a non-empty execution state.
    ///
    /// Singleton eligibility is representation-based: this retains exactly one root, but does not
    /// verify execution provenance.
    pub fn new(state: DeferredState) -> Result<Self, PrecompileWitnessError> {
        let root = state.root();
        if root == TRUE_DIGEST {
            return Err(PrecompileWitnessError::TrueRoot);
        }

        let witness = Self { state, roots: alloc::vec![root] };
        witness.validate()?;
        Ok(witness)
    }

    /// Returns the aggregate root authenticated by the hydrated deferred state.
    pub fn root(&self) -> Digest {
        self.state.root()
    }

    /// Returns the ordered non-TRUE execution roots represented by this witness.
    pub fn roots(&self) -> &[Digest] {
        &self.roots
    }

    /// Returns the hydrated deferred state used by precompile proving.
    pub fn state(&self) -> &DeferredState {
        &self.state
    }

    /// Consumes this witness and returns the hydrated deferred state used by precompile proving.
    pub fn into_state(self) -> DeferredState {
        self.state
    }

    /// Merges ordered singleton witnesses into one aggregate witness.
    ///
    /// All inputs are validated before any deferred state is consumed. Singleton eligibility means
    /// `roots.len() == 1`; execution provenance is not encoded or verified. Witnesses retaining
    /// multiple roots are rejected, so roots cannot be regrouped or merged recursively. The
    /// complete merged state is bounded by [`super::MAX_DEFERRED_ELEMENTS`].
    pub fn merge(witnesses: Vec<Self>) -> Result<Self, PrecompileWitnessError> {
        if witnesses.is_empty() {
            return Err(PrecompileWitnessError::EmptyMerge);
        }
        if witnesses.len() > MAX_PRECOMPILE_ROOTS {
            return Err(PrecompileWitnessError::TooManyRoots {
                roots: witnesses.len(),
                max: MAX_PRECOMPILE_ROOTS,
            });
        }

        for witness in &witnesses {
            witness.validate_singleton()?;
        }

        let roots = witnesses.iter().map(|witness| witness.roots[0]).collect::<Vec<_>>();
        let mut witnesses = witnesses.into_iter();
        let mut state = witnesses.next().expect("non-empty witness input was checked above").state;

        for witness in witnesses {
            state = state.merge(witness.state).map_err(PrecompileWitnessError::Merge)?;
        }

        let expected_root = roots
            .iter()
            .copied()
            .reduce(fold_deferred_root)
            .expect("non-empty witness input retains at least one root");
        if state.root() != expected_root {
            return Err(PrecompileWitnessError::RootMismatch);
        }

        Ok(Self { state, roots })
    }

    /// Encodes this witness using the canonical root-reachable [`DeferredStateWire`]
    /// representation.
    ///
    /// Encoding preserves the witness representation; it does not establish witness validity.
    pub fn to_bytes(&self) -> Result<Vec<u8>, PrecompileWitnessError> {
        let wire = self.state.to_wire()?;
        let mut bytes = Vec::new();
        bytes.write_usize(self.roots.len());
        for root in &self.roots {
            root.write_into(&mut bytes);
        }
        wire.write_into(&mut bytes);
        Ok(bytes)
    }

    /// Decodes and rehydrates a witness under the supplied registry and fixed deferred-element
    /// ceiling.
    pub fn from_bytes(
        bytes: &[u8],
        registry: Arc<PrecompileRegistry>,
    ) -> Result<Self, PrecompileWitnessError> {
        let mut reader = BudgetedReader::new(SliceReader::new(bytes), bytes.len());
        let witness = Self::read_from(&mut reader, registry)?;
        if reader.has_more_bytes() {
            return Err(DeserializationError::InvalidValue(
                "extra bytes after precompile witness payload".into(),
            )
            .into());
        }
        if witness.to_bytes()? != bytes {
            return Err(PrecompileWitnessError::NonCanonicalEncoding);
        }
        Ok(witness)
    }

    /// Decodes one witness payload from `source` using explicit hydration context.
    ///
    /// Canonicality of a nested payload is checked by the enclosing transport after it has consumed
    /// its complete byte slice.
    pub(crate) fn read_from<R: ByteReader>(
        source: &mut R,
        registry: Arc<PrecompileRegistry>,
    ) -> Result<Self, PrecompileWitnessError> {
        let root_count = source.read_usize()?;
        if root_count > MAX_PRECOMPILE_ROOTS {
            return Err(PrecompileWitnessError::TooManyRoots {
                roots: root_count,
                max: MAX_PRECOMPILE_ROOTS,
            });
        }
        let roots = source.read_many_iter::<Digest>(root_count)?.collect::<Result<Vec<_>, _>>()?;
        let wire = DeferredStateWire::read_from(source)?;
        let state = DeferredState::from_wire(registry, &wire)?;
        let witness = Self { state, roots };
        witness.validate()?;
        Ok(witness)
    }

    fn validate_singleton(&self) -> Result<(), PrecompileWitnessError> {
        if self.roots.len() != 1 {
            return Err(PrecompileWitnessError::NonSingleton);
        }
        self.validate()
    }

    pub(crate) fn validate(&self) -> Result<(), PrecompileWitnessError> {
        if self.roots.is_empty() {
            return Err(PrecompileWitnessError::NoRoots);
        }
        if self.roots.contains(&TRUE_DIGEST) {
            return Err(PrecompileWitnessError::TrueRoot);
        }
        if self.roots.iter().any(|root| self.state.get_node(root).is_none()) {
            return Err(PrecompileWitnessError::RootMismatch);
        }

        let expected_root = self
            .roots
            .iter()
            .copied()
            .reduce(fold_deferred_root)
            .expect("non-empty roots were checked above");
        if self.state.root() != expected_root {
            return Err(PrecompileWitnessError::RootMismatch);
        }

        Ok(())
    }
}

/// Errors produced while constructing, merging, or decoding precompile witnesses.
#[derive(Debug, thiserror::Error)]
pub enum PrecompileWitnessError {
    /// A witness cannot represent an empty deferred execution.
    #[error("precompile witness roots must differ from TRUE_DIGEST")]
    TrueRoot,
    /// A decoded witness must retain at least one execution root.
    #[error("precompile witness must retain at least one root")]
    NoRoots,
    /// A merge operation requires at least one singleton witness.
    #[error("cannot merge an empty precompile witness list")]
    EmptyMerge,
    /// The ordered root sequence exceeds the hard allocation and folding safety ceiling.
    #[error("precompile witness contains too many roots: found {roots}, maximum is {max}")]
    TooManyRoots { roots: usize, max: usize },
    /// Merge inputs must come directly from individual executions.
    #[error("precompile witness merge inputs must each contain exactly one root")]
    NonSingleton,
    /// The retained roots do not describe the hydrated deferred state.
    #[error("precompile witness roots do not match its deferred state")]
    RootMismatch,
    /// The decoded bytes are not the unique canonical witness encoding.
    #[error("precompile witness bytes are not canonically encoded")]
    NonCanonicalEncoding,
    /// Sequential deferred-state aggregation failed.
    #[error("failed to merge deferred precompile states: {0}")]
    Merge(#[source] PrecompileError),
    /// Canonical deferred-state encoding or rehydration failed.
    #[error(transparent)]
    Integrity(#[from] IntegrityError),
    /// The witness byte representation is malformed.
    #[error(transparent)]
    Deserialization(#[from] DeserializationError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Felt, ZERO,
        deferred::{
            DeferredContext, MAX_DEFERRED_ELEMENTS, Node, NodeType, Payload, Precompile, Tag,
            precompile_id,
        },
    };

    fn framework_state(statement_depth: usize) -> DeferredState {
        let mut state = DeferredState::default();
        let mut statement = TRUE_DIGEST;
        for _ in 0..statement_depth {
            statement = state.register(Node::and(statement, TRUE_DIGEST)).unwrap();
        }
        state.log_statement(statement).unwrap();
        state
    }

    fn singleton(statement_depth: usize) -> PrecompileWitness {
        PrecompileWitness::new(framework_state(statement_depth)).unwrap()
    }

    fn encoded_parts(roots: &[Digest], wire: &DeferredStateWire) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.write_usize(roots.len());
        for root in roots {
            root.write_into(&mut bytes);
        }
        wire.write_into(&mut bytes);
        bytes
    }

    #[derive(Debug, Clone, Copy)]
    struct FixturePrecompile;

    impl FixturePrecompile {
        const NAME: &'static str = "precompile-witness-fixture";

        fn tag() -> Tag {
            Tag::precompile(precompile_id(Self::NAME), [ZERO; 3])
                .expect("fixture id is precompile-owned")
        }
    }

    impl Precompile for FixturePrecompile {
        fn name(&self) -> &'static str {
            Self::NAME
        }

        fn id(&self) -> Felt {
            precompile_id(self.name())
        }

        fn decode(&self, args: [Felt; 3]) -> Option<NodeType> {
            (args == [ZERO; 3]).then_some(NodeType::Data)
        }

        fn evaluate(
            &self,
            _args: [Felt; 3],
            _payload: &Payload,
            _context: &mut DeferredContext<'_>,
        ) -> Result<Node, PrecompileError> {
            Ok(Node::TRUE)
        }
    }

    fn fixture_witness() -> PrecompileWitness {
        let registry = Arc::new(PrecompileRegistry::new().with_precompile(FixturePrecompile));
        let mut state = DeferredState::new(registry).unwrap();
        let statement = state
            .register(Node::value(FixturePrecompile::tag(), [ZERO; 8]).unwrap())
            .unwrap();
        state.log_statement(statement).unwrap();
        PrecompileWitness::new(state).unwrap()
    }

    #[test]
    fn singleton_construction_rejects_true_and_retains_execution_root() {
        assert!(matches!(
            PrecompileWitness::new(DeferredState::default()),
            Err(PrecompileWitnessError::TrueRoot)
        ));

        let state = framework_state(1);
        let root = state.root();
        let witness = PrecompileWitness::new(state).unwrap();

        assert_eq!(witness.root(), root);
        assert_eq!(witness.roots(), &[root]);
        assert_eq!(witness.state().root(), root);
    }

    #[test]
    fn merge_rejects_empty_input() {
        assert!(matches!(
            PrecompileWitness::merge(Vec::new()),
            Err(PrecompileWitnessError::EmptyMerge)
        ));
    }

    #[test]
    fn merge_preserves_order_and_changes_the_aggregate_when_reversed() {
        let first = singleton(1);
        let second = singleton(2);
        let first_root = first.root();
        let second_root = second.root();

        let ordered = PrecompileWitness::merge(alloc::vec![first.clone(), second.clone()]).unwrap();
        let reversed = PrecompileWitness::merge(alloc::vec![second, first]).unwrap();

        assert_eq!(ordered.roots(), &[first_root, second_root]);
        assert_eq!(ordered.root(), fold_deferred_root(first_root, second_root));
        assert_eq!(reversed.roots(), &[second_root, first_root]);
        assert_eq!(reversed.root(), fold_deferred_root(second_root, first_root));
        assert_ne!(ordered.root(), reversed.root());
    }

    #[test]
    fn merge_preserves_duplicate_singleton_roots() {
        let witness = singleton(1);
        let root = witness.root();

        let merged = PrecompileWitness::merge(alloc::vec![witness.clone(), witness]).unwrap();

        assert_eq!(merged.roots(), &[root, root]);
        assert_eq!(merged.root(), fold_deferred_root(root, root));
    }

    #[test]
    fn merge_rejects_an_already_merged_input_during_prevalidation() {
        let merged = PrecompileWitness::merge(alloc::vec![singleton(1), singleton(2)]).unwrap();

        let error = PrecompileWitness::merge(alloc::vec![singleton(3), fixture_witness(), merged])
            .unwrap_err();

        assert!(matches!(error, PrecompileWitnessError::NonSingleton));
    }

    #[test]
    fn one_element_merge_is_a_singleton_identity() {
        let witness = singleton(1);
        let root = witness.root();
        let num_elements = witness.state().num_elements();

        let merged = PrecompileWitness::merge(alloc::vec![witness]).unwrap();

        assert_eq!(merged.root(), root);
        assert_eq!(merged.roots(), &[root]);
        assert_eq!(merged.state().num_elements(), num_elements);
        assert_eq!(merged.state().remaining_elements(), MAX_DEFERRED_ELEMENTS - num_elements);
    }

    #[test]
    fn merge_rejects_excessive_root_count() {
        let witnesses = alloc::vec![singleton(1); MAX_PRECOMPILE_ROOTS + 1];

        assert!(matches!(
            PrecompileWitness::merge(witnesses),
            Err(PrecompileWitnessError::TooManyRoots {
                roots,
                max: MAX_PRECOMPILE_ROOTS,
            }) if roots == MAX_PRECOMPILE_ROOTS + 1
        ));
    }

    #[test]
    fn wire_decode_rejects_excessive_root_count_before_hydration() {
        let mut bytes = Vec::new();
        bytes.write_usize(MAX_PRECOMPILE_ROOTS + 1);

        assert!(matches!(
            PrecompileWitness::from_bytes(&bytes, Arc::new(PrecompileRegistry::new())),
            Err(PrecompileWitnessError::TooManyRoots {
                roots,
                max: MAX_PRECOMPILE_ROOTS,
            }) if roots == MAX_PRECOMPILE_ROOTS + 1
        ));
    }

    #[test]
    fn wire_round_trip_preserves_aggregate_state_and_ordered_roots() {
        let witness = PrecompileWitness::merge(alloc::vec![singleton(1), singleton(2)]).unwrap();
        let bytes = witness.to_bytes().unwrap();

        let decoded =
            PrecompileWitness::from_bytes(&bytes, Arc::new(PrecompileRegistry::new())).unwrap();

        assert_eq!(decoded.root(), witness.root());
        assert_eq!(decoded.roots(), witness.roots());
    }

    #[test]
    fn wire_decode_rejects_non_minimal_vint_length_encoding() {
        let witness = singleton(1);
        let canonical = witness.to_bytes().unwrap();
        assert_eq!(canonical[0], 3, "one root uses the canonical one-byte vint encoding");

        let mut noncanonical = Vec::with_capacity(canonical.len() + 8);
        noncanonical.push(0); // Nine-byte vint marker.
        noncanonical.extend_from_slice(&1u64.to_le_bytes());
        noncanonical.extend_from_slice(&canonical[1..]);

        assert!(matches!(
            PrecompileWitness::from_bytes(&noncanonical, Arc::new(PrecompileRegistry::new())),
            Err(PrecompileWitnessError::NonCanonicalEncoding)
        ));
    }

    #[test]
    fn context_aware_wire_decode_rejects_invalid_inputs() {
        let registry = Arc::new(PrecompileRegistry::new());
        assert!(matches!(
            PrecompileWitness::from_bytes(&[0], Arc::clone(&registry)),
            Err(PrecompileWitnessError::Deserialization(_))
        ));

        let witness = singleton(1);
        let mut noncanonical_wire = witness.state().to_wire().unwrap();
        noncanonical_wire.entries.push(
            noncanonical_wire
                .entries
                .last()
                .expect("non-empty state has a wire entry")
                .clone(),
        );
        let noncanonical = encoded_parts(witness.roots(), &noncanonical_wire);
        assert!(matches!(
            PrecompileWitness::from_bytes(&noncanonical, Arc::clone(&registry)),
            Err(PrecompileWitnessError::Integrity(IntegrityError::InvalidStructure))
        ));

        let fixture_bytes = fixture_witness().to_bytes().unwrap();
        assert!(matches!(
            PrecompileWitness::from_bytes(&fixture_bytes, registry),
            Err(PrecompileWitnessError::Integrity(IntegrityError::InvalidStructure))
        ));
    }

    #[test]
    fn wire_decode_rejects_true_roots_and_trailing_bytes() {
        let empty_wire = DeferredStateWire::default();
        let true_root = encoded_parts(&[TRUE_DIGEST], &empty_wire);
        assert!(matches!(
            PrecompileWitness::from_bytes(&true_root, Arc::new(PrecompileRegistry::new())),
            Err(PrecompileWitnessError::TrueRoot)
        ));

        let mut trailing = singleton(1).to_bytes().unwrap();
        trailing.push(0);
        assert!(matches!(
            PrecompileWitness::from_bytes(&trailing, Arc::new(PrecompileRegistry::new())),
            Err(PrecompileWitnessError::Deserialization(_))
        ));
    }
}
