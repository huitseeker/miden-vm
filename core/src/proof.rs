use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

#[cfg(feature = "arbitrary")]
use proptest::prelude::*;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    crypto::hash::{Blake3_256, Poseidon2, Rpo256, Rpx256},
    deferred::{
        DeferredRoot, MAX_PRECOMPILE_ROOTS, PrecompileRegistry, PrecompileWitness,
        PrecompileWitnessError, TRUE_DIGEST,
    },
    serde::{
        BudgetedReader, ByteReader, ByteWriter, Deserializable, DeserializationError, Serializable,
        SliceReader,
    },
};

// HASH FUNCTION
// ================================================================================================

/// A hash function used during STARK proof generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(
    all(feature = "arbitrary", test),
    miden_test_serde_macros::serde_test(binary_serde(true))
)]
#[repr(u8)]
pub enum HashFunction {
    /// BLAKE3 hash function with 256-bit output.
    Blake3_256 = 0x01,
    /// RPO hash function with 256-bit output.
    Rpo256 = 0x02,
    /// RPX hash function with 256-bit output.
    Rpx256 = 0x03,
    /// Poseidon2 hash function with 256-bit output.
    Poseidon2 = 0x04,
    /// Keccak hash function with 256-bit output.
    Keccak = 0x05,
}

impl HashFunction {
    /// Returns the collision resistance level (in bits) of this hash function.
    pub const fn collision_resistance(&self) -> u32 {
        match self {
            HashFunction::Blake3_256 => Blake3_256::COLLISION_RESISTANCE,
            HashFunction::Rpo256 => Rpo256::COLLISION_RESISTANCE,
            HashFunction::Rpx256 => Rpx256::COLLISION_RESISTANCE,
            HashFunction::Poseidon2 => Poseidon2::COLLISION_RESISTANCE,
            HashFunction::Keccak => 128,
        }
    }
}

/// Error type for invalid hash function strings.
#[derive(Debug, thiserror::Error)]
#[error(
    "invalid hash function '{hash_function}'. Valid options are: blake3-256, rpo, rpx, poseidon2, keccak"
)]
pub struct InvalidHashFunctionError {
    pub hash_function: String,
}

impl TryFrom<u8> for HashFunction {
    type Error = DeserializationError;

    fn try_from(repr: u8) -> Result<Self, Self::Error> {
        match repr {
            0x01 => Ok(Self::Blake3_256),
            0x02 => Ok(Self::Rpo256),
            0x03 => Ok(Self::Rpx256),
            0x04 => Ok(Self::Poseidon2),
            0x05 => Ok(Self::Keccak),
            _ => Err(DeserializationError::InvalidValue(format!(
                "the hash function representation {repr} is not valid!"
            ))),
        }
    }
}

impl TryFrom<&str> for HashFunction {
    type Error = InvalidHashFunctionError;

    fn try_from(hash_fn_str: &str) -> Result<Self, Self::Error> {
        match hash_fn_str {
            "blake3-256" => Ok(Self::Blake3_256),
            "rpo" => Ok(Self::Rpo256),
            "rpx" => Ok(Self::Rpx256),
            "poseidon2" => Ok(Self::Poseidon2),
            "keccak" => Ok(Self::Keccak),
            _ => Err(InvalidHashFunctionError { hash_function: hash_fn_str.to_string() }),
        }
    }
}

#[cfg(feature = "arbitrary")]
impl Arbitrary for HashFunction {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        any::<u8>()
            .prop_map(|tag| match tag % 5 {
                0 => Self::Blake3_256,
                1 => Self::Rpo256,
                2 => Self::Rpx256,
                3 => Self::Poseidon2,
                _ => Self::Keccak,
            })
            .boxed()
    }
}

impl Serializable for HashFunction {
    fn write_into<W: ByteWriter>(&self, target: &mut W) {
        target.write_u8(*self as u8);
    }
}

impl Deserializable for HashFunction {
    fn read_from<R: ByteReader>(source: &mut R) -> Result<Self, DeserializationError> {
        source.read_u8()?.try_into()
    }
}

// PROOF ARTIFACTS
// ================================================================================================

/// Hard encoded-size and per-allocation safety ceiling for every STARK proof.
pub const MAX_STARK_PROOF_BYTES: usize = 64 * 1024 * 1024;

/// A Miden VM STARK proof together with its authenticated precompile obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmProof {
    proof: StarkProof,
    precompile_root: DeferredRoot,
}

impl VmProof {
    /// Creates a VM proof artifact from its STARK proof and authenticated precompile root.
    pub const fn from_parts(proof: StarkProof, precompile_root: DeferredRoot) -> Self {
        Self { proof, precompile_root }
    }

    /// Returns the VM STARK proof.
    pub const fn proof(&self) -> &StarkProof {
        &self.proof
    }

    /// Returns the authenticated precompile obligation.
    pub const fn precompile_root(&self) -> DeferredRoot {
        self.precompile_root
    }

    /// Consumes this artifact and returns its STARK proof and authenticated precompile root.
    pub fn into_parts(self) -> (StarkProof, DeferredRoot) {
        (self.proof, self.precompile_root)
    }
}

impl Serializable for VmProof {
    fn write_into<W: ByteWriter>(&self, target: &mut W) {
        self.proof.write_into(target);
        self.precompile_root.write_into(target);
    }
}

impl Deserializable for VmProof {
    fn read_from<R: ByteReader>(source: &mut R) -> Result<Self, DeserializationError> {
        let proof = StarkProof::read_from(source)?;
        let precompile_root = DeferredRoot::read_from(source)?;
        Ok(Self::from_parts(proof, precompile_root))
    }

    fn read_from_bytes(bytes: &[u8]) -> Result<Self, DeserializationError> {
        let mut reader = BudgetedReader::new(SliceReader::new(bytes), bytes.len());
        Self::read_from(&mut reader)
    }

    fn min_serialized_size() -> usize {
        StarkProof::min_serialized_size() + DeferredRoot::min_serialized_size()
    }
}

/// A precompile STARK proof with the ordered, non-empty roots that form its aggregate root.
///
/// Verification folds the entire sequence. [`TRUE_DIGEST`] roots are omitted; order and duplicate
/// multiplicity are significant.
///
/// Binary decoding enforces the fixed root ceiling before reserving root storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrecompileProof {
    proof: StarkProof,
    roots: Vec<DeferredRoot>,
}

impl PrecompileProof {
    /// Creates a precompile proof artifact after validating its constituent root shape.
    ///
    /// This does not verify the STARK proof.
    pub fn from_parts(
        proof: StarkProof,
        roots: Vec<DeferredRoot>,
    ) -> Result<Self, ExecutionProofError> {
        if roots.is_empty() {
            return Err(ExecutionProofError::EmptyPrecompileRoots);
        }
        if roots.len() > MAX_PRECOMPILE_ROOTS {
            return Err(ExecutionProofError::TooManyPrecompileRoots {
                roots: roots.len(),
                max: MAX_PRECOMPILE_ROOTS,
            });
        }
        if let Some(index) = roots.iter().position(|root| *root == TRUE_DIGEST) {
            return Err(ExecutionProofError::SettledPrecompileRoot { index });
        }

        Ok(Self { proof, roots })
    }

    /// Returns the precompile STARK proof.
    pub const fn proof(&self) -> &StarkProof {
        &self.proof
    }

    /// Returns all constituent roots in fold order.
    pub fn roots(&self) -> &[DeferredRoot] {
        &self.roots
    }

    /// Returns the aggregate root obtained by folding all constituent roots in order.
    pub fn aggregate_root(&self) -> DeferredRoot {
        self.roots
            .iter()
            .copied()
            .reduce(crate::deferred::fold_deferred_root)
            .expect("PrecompileProof roots are non-empty by construction")
    }

    /// Consumes this artifact and returns its STARK proof and ordered constituent roots.
    pub fn into_parts(self) -> (StarkProof, Vec<DeferredRoot>) {
        (self.proof, self.roots)
    }

    fn covers<'a>(&self, required_roots: impl Iterator<Item = &'a DeferredRoot>) -> bool {
        let mut proof_roots = self.roots.iter();
        required_roots
            .filter(|root| **root != TRUE_DIGEST)
            .all(|required| proof_roots.any(|provided| provided == required))
    }
}

impl Serializable for PrecompileProof {
    fn write_into<W: ByteWriter>(&self, target: &mut W) {
        self.proof.write_into(target);
        self.roots.write_into(target);
    }
}

impl Deserializable for PrecompileProof {
    fn read_from<R: ByteReader>(source: &mut R) -> Result<Self, DeserializationError> {
        let proof = StarkProof::read_from(source)?;
        let root_count = source.read_usize()?;
        if root_count > MAX_PRECOMPILE_ROOTS {
            return Err(invalid_proof_shape(ExecutionProofError::TooManyPrecompileRoots {
                roots: root_count,
                max: MAX_PRECOMPILE_ROOTS,
            }));
        }
        let roots = source
            .read_many_iter::<DeferredRoot>(root_count)?
            .collect::<Result<Vec<_>, _>>()?;
        Self::from_parts(proof, roots).map_err(invalid_proof_shape)
    }

    fn read_from_bytes(bytes: &[u8]) -> Result<Self, DeserializationError> {
        let mut reader = BudgetedReader::new(SliceReader::new(bytes), bytes.len());
        Self::read_from(&mut reader)
    }

    fn min_serialized_size() -> usize {
        StarkProof::min_serialized_size()
            + usize::min_serialized_size()
            + DeferredRoot::min_serialized_size()
    }
}

const DEFERRED_PROOF_DISCRIMINANT: u8 = 0;
const COMPLETE_PROOF_DISCRIMINANT: u8 = 1;

/// A Miden VM execution proof, either awaiting precompile proving or complete.
///
/// The public variants may represent inconsistent artifacts. Checked constructors are conveniences;
/// only full verification establishes proof validity.
#[derive(Debug, Clone)]
pub enum ExecutionProof {
    /// The VM STARK is available and its singleton precompile witness remains to be proved.
    Deferred {
        vm: VmProof,
        precompile: PrecompileWitness,
    },
    /// All proof work is complete. `None` means the VM authenticated [`TRUE_DIGEST`].
    Complete {
        vm: VmProof,
        precompile: Option<PrecompileProof>,
    },
}

impl ExecutionProof {
    /// Creates a deferred proof after checking the VM root against a singleton witness.
    pub fn new_deferred(
        vm: VmProof,
        precompile: PrecompileWitness,
    ) -> Result<Self, ExecutionProofError> {
        validate_deferred(&vm, &precompile)?;
        Ok(Self::Deferred { vm, precompile })
    }

    /// Creates a complete proof after checking its optional precompile proof covers the VM root.
    ///
    /// This only checks artifact structure; it does not verify either STARK proof.
    pub fn new_complete(
        vm: VmProof,
        precompile: Option<PrecompileProof>,
    ) -> Result<Self, ExecutionProofError> {
        validate_complete(&vm, precompile.as_ref())?;
        Ok(Self::Complete { vm, precompile })
    }

    /// Checks that this proof's artifacts have a compatible structure.
    ///
    /// This does not cryptographically verify either STARK proof.
    pub fn validate_structure(&self) -> Result<(), ExecutionProofError> {
        match self {
            Self::Deferred { vm, precompile } => validate_deferred(vm, precompile),
            Self::Complete { vm, precompile } => validate_complete(vm, precompile.as_ref()),
        }
    }

    /// Returns whether this value has the [`Self::Complete`] representation.
    ///
    /// This is a shape check only. Public variants may be inconsistent; use full verification to
    /// establish validity and determine whether an authenticated obligation remains outstanding.
    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete { .. })
    }

    /// Returns the VM proof artifact in either state.
    pub const fn vm(&self) -> &VmProof {
        match self {
            Self::Deferred { vm, .. } | Self::Complete { vm, .. } => vm,
        }
    }

    /// Returns the retained witness when this proof is deferred.
    ///
    /// Call `.cloned()` on the returned option when ownership of the witness is needed without
    /// consuming the VM proof required for completion.
    pub const fn precompile_witness(&self) -> Option<&PrecompileWitness> {
        match self {
            Self::Deferred { precompile, .. } => Some(precompile),
            Self::Complete { .. } => None,
        }
    }

    /// Returns the completed precompile proof, if this is a complete proof that carries one.
    pub const fn precompile(&self) -> Option<&PrecompileProof> {
        match self {
            Self::Complete { precompile, .. } => precompile.as_ref(),
            Self::Deferred { .. } => None,
        }
    }

    /// Attaches a compatible precompile proof to a deferred proof.
    pub fn complete(self, precompile: PrecompileProof) -> Result<Self, ExecutionProofError> {
        let Self::Deferred { vm, precompile: witness } = self else {
            return Err(ExecutionProofError::AlreadyComplete);
        };
        validate_deferred(&vm, &witness)?;
        validate_complete(&vm, Some(&precompile))?;
        Ok(Self::Complete { vm, precompile: Some(precompile) })
    }

    /// Encodes either state canonically.
    ///
    /// Encoding preserves the public enum representation and does not establish proof validity.
    /// Use [`Self::read_from_bytes`] with an explicit registry to decode.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ExecutionProofTransportError> {
        let mut bytes = Vec::new();
        match self {
            Self::Deferred { vm, precompile } => {
                bytes.write_u8(DEFERRED_PROOF_DISCRIMINANT);
                vm.write_into(&mut bytes);
                bytes.write_bytes(&precompile.to_bytes()?);
            },
            Self::Complete { vm, precompile } => {
                bytes.write_u8(COMPLETE_PROOF_DISCRIMINANT);
                vm.write_into(&mut bytes);
                precompile.write_into(&mut bytes);
            },
        }
        Ok(bytes)
    }

    /// Decodes an execution proof with the context required to hydrate a deferred witness.
    ///
    /// `registry` defines the installed precompile semantics. Hydrated deferred-state material is
    /// bounded by the library's fixed element ceiling. Complete proofs do not otherwise depend on
    /// this context. Decoding establishes transport syntax, canonical representation, and witness
    /// hydration, not full proof validity.
    pub fn read_from_bytes(
        bytes: &[u8],
        registry: Arc<PrecompileRegistry>,
    ) -> Result<Self, ExecutionProofTransportError> {
        let mut reader = BudgetedReader::new(SliceReader::new(bytes), bytes.len());
        let discriminant = reader.read_u8()?;
        if !matches!(discriminant, DEFERRED_PROOF_DISCRIMINANT | COMPLETE_PROOF_DISCRIMINANT) {
            return Err(DeserializationError::InvalidValue(format!(
                "invalid execution proof discriminant {discriminant}"
            ))
            .into());
        }

        let vm = VmProof::read_from(&mut reader)?;
        let proof = match discriminant {
            DEFERRED_PROOF_DISCRIMINANT => {
                let precompile = PrecompileWitness::read_from(&mut reader, registry)?;
                Self::Deferred { vm, precompile }
            },
            COMPLETE_PROOF_DISCRIMINANT => {
                let precompile = Option::<PrecompileProof>::read_from(&mut reader)?;
                Self::Complete { vm, precompile }
            },
            _ => unreachable!("execution proof discriminant was checked before decoding"),
        };

        if reader.has_more_bytes() {
            return Err(DeserializationError::InvalidValue(
                "extra bytes after execution proof payload".into(),
            )
            .into());
        }
        if proof.to_bytes()? != bytes {
            return Err(ExecutionProofTransportError::NonCanonicalEncoding);
        }

        Ok(proof)
    }
}

/// Structural errors returned while constructing or composing proof artifacts.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExecutionProofError {
    /// A precompile proof must identify at least one constituent root.
    #[error("a precompile proof must contain at least one constituent root")]
    EmptyPrecompileRoots,
    /// The ordered root sequence exceeds the hard allocation and folding safety ceiling.
    #[error("precompile proof contains too many roots: found {roots}, maximum is {max}")]
    TooManyPrecompileRoots { roots: usize, max: usize },
    /// Settled roots are omitted from precompile proof constituent roots.
    #[error("precompile proof constituent root at index {index} is already settled")]
    SettledPrecompileRoot { index: usize },
    /// A deferred proof must authenticate outstanding precompile work.
    #[error("a deferred execution proof cannot authenticate TRUE_DIGEST")]
    DeferredTrueRoot,
    /// A deferred proof must retain exactly one execution witness root.
    #[error("a deferred execution proof witness must contain exactly one root, found {roots}")]
    DeferredNonSingletonWitness { roots: usize },
    /// The deferred witness root must equal the root authenticated by the VM proof.
    #[error("the deferred witness root does not match the VM precompile root")]
    DeferredRootMismatch,
    /// A precompile proof was supplied for an already settled VM obligation.
    #[error("a precompile proof was supplied for an already settled VM obligation")]
    UnexpectedPrecompileProof,
    /// A non-empty VM obligation had no precompile proof.
    #[error("a precompile proof is required for a non-empty VM obligation")]
    MissingPrecompileProof,
    /// The proof roots do not cover the VM obligations as an ordered subsequence.
    #[error("precompile proof roots do not cover VM obligations in order")]
    InsufficientPrecompileRootCoverage,
    /// Only a deferred execution proof can be completed.
    #[error("the execution proof is already complete")]
    AlreadyComplete,
}

/// Errors returned by canonical execution-proof encoding and context-aware decoding.
#[derive(Debug, thiserror::Error)]
pub enum ExecutionProofTransportError {
    /// Deferred witness encoding or hydration failed.
    #[error(transparent)]
    Witness(#[from] PrecompileWitnessError),
    /// The byte representation is malformed.
    #[error(transparent)]
    Deserialization(#[from] DeserializationError),
    /// The decoded representation was valid but not encoded uniquely.
    #[error("execution proof bytes are not canonically encoded")]
    NonCanonicalEncoding,
}

fn validate_deferred(
    vm: &VmProof,
    precompile: &PrecompileWitness,
) -> Result<(), ExecutionProofError> {
    if vm.precompile_root == TRUE_DIGEST {
        return Err(ExecutionProofError::DeferredTrueRoot);
    }
    if precompile.roots().len() != 1 {
        return Err(ExecutionProofError::DeferredNonSingletonWitness {
            roots: precompile.roots().len(),
        });
    }
    if precompile.validate().is_err() || precompile.roots()[0] != vm.precompile_root {
        return Err(ExecutionProofError::DeferredRootMismatch);
    }
    Ok(())
}

fn validate_complete(
    vm: &VmProof,
    precompile: Option<&PrecompileProof>,
) -> Result<(), ExecutionProofError> {
    validate_precompile_coverage(core::iter::once(&vm.precompile_root), precompile)
}

fn validate_precompile_coverage<'a>(
    required_roots: impl Iterator<Item = &'a DeferredRoot>,
    precompile: Option<&PrecompileProof>,
) -> Result<(), ExecutionProofError> {
    let mut required_roots = required_roots.filter(|root| **root != TRUE_DIGEST).peekable();

    if required_roots.peek().is_none() {
        return if precompile.is_none() {
            Ok(())
        } else {
            Err(ExecutionProofError::UnexpectedPrecompileProof)
        };
    }

    let precompile = precompile.ok_or(ExecutionProofError::MissingPrecompileProof)?;
    if precompile.covers(required_roots) {
        Ok(())
    } else {
        Err(ExecutionProofError::InsufficientPrecompileRootCoverage)
    }
}

fn invalid_proof_shape(error: ExecutionProofError) -> DeserializationError {
    DeserializationError::InvalidValue(error.to_string())
}

// STARK PROOF
// ================================================================================================

/// A serialized STARK proof and the hash function used during proof generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StarkProof {
    bytes: Vec<u8>,
    hash_fn: HashFunction,
}

impl StarkProof {
    /// Creates a new instance of [StarkProof] from proof bytes and hash function.
    pub const fn new(bytes: Vec<u8>, hash_fn: HashFunction) -> Self {
        Self { bytes, hash_fn }
    }

    /// Returns the serialized STARK proof bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the hash function used during proof generation process.
    pub const fn hash_fn(&self) -> HashFunction {
        self.hash_fn
    }

    /// Returns the serialized STARK proof bytes and hash function.
    pub fn into_parts(self) -> (Vec<u8>, HashFunction) {
        (self.bytes, self.hash_fn)
    }
}

impl Serializable for StarkProof {
    fn write_into<W: ByteWriter>(&self, target: &mut W) {
        self.bytes.write_into(target);
        self.hash_fn.write_into(target);
    }
}

impl Deserializable for StarkProof {
    fn read_from<R: ByteReader>(source: &mut R) -> Result<Self, DeserializationError> {
        let byte_count = source.read_usize()?;
        if byte_count > MAX_STARK_PROOF_BYTES {
            return Err(DeserializationError::InvalidValue(format!(
                "STARK proof contains too many bytes: found {byte_count}, maximum is {MAX_STARK_PROOF_BYTES}"
            )));
        }
        let bytes = source.read_many_iter::<u8>(byte_count)?.collect::<Result<Vec<_>, _>>()?;
        let hash_fn = HashFunction::read_from(source)?;
        Ok(Self::new(bytes, hash_fn))
    }

    fn read_from_bytes(bytes: &[u8]) -> Result<Self, DeserializationError> {
        let mut reader = BudgetedReader::new(SliceReader::new(bytes), bytes.len());
        Self::read_from(&mut reader)
    }

    fn min_serialized_size() -> usize {
        Vec::<u8>::min_serialized_size() + HashFunction::min_serialized_size()
    }
}

#[cfg(test)]
mod tests {
    use core::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::{
        Felt,
        deferred::{
            DeferredContext, DeferredState, IntegrityError, Node, NodeType, Payload, Precompile,
            PrecompileError, Tag, precompile_id,
        },
        serde::ByteWriter,
    };

    fn dummy_stark_proof(bytes: &[u8]) -> StarkProof {
        StarkProof::new(bytes.to_vec(), HashFunction::Blake3_256)
    }

    fn root(value: u64) -> DeferredRoot {
        [
            Felt::new(value).unwrap(),
            Felt::new(0).unwrap(),
            Felt::new(0).unwrap(),
            Felt::new(0).unwrap(),
        ]
        .into()
    }

    fn vm_proof(precompile_root: DeferredRoot) -> VmProof {
        VmProof::from_parts(dummy_stark_proof(&[1]), precompile_root)
    }

    fn precompile_proof(roots: &[DeferredRoot]) -> PrecompileProof {
        PrecompileProof::from_parts(dummy_stark_proof(&[2]), roots.to_vec()).unwrap()
    }

    fn witness(statement_depth: usize) -> PrecompileWitness {
        let mut state = DeferredState::default();
        let mut statement = TRUE_DIGEST;
        for _ in 0..statement_depth {
            statement = state.register(Node::and(statement, TRUE_DIGEST)).unwrap();
        }
        state.log_statement(statement).unwrap();
        PrecompileWitness::new(state).unwrap()
    }

    fn registry() -> Arc<PrecompileRegistry> {
        Arc::new(PrecompileRegistry::new())
    }

    #[derive(Debug, Clone)]
    struct MutableShapeFixture {
        data_shape: Arc<AtomicBool>,
    }

    impl MutableShapeFixture {
        const NAME: &'static str = "proof-mutable-shape-fixture";

        fn tag() -> Tag {
            Tag::precompile(precompile_id(Self::NAME), [Felt::ZERO; 3])
                .expect("fixture id is precompile-owned")
        }
    }

    impl Precompile for MutableShapeFixture {
        fn name(&self) -> &'static str {
            Self::NAME
        }

        fn id(&self) -> Felt {
            precompile_id(Self::NAME)
        }

        fn decode(&self, args: [Felt; 3]) -> Option<NodeType> {
            (args == [Felt::ZERO; 3]).then(|| {
                if self.data_shape.load(Ordering::Relaxed) {
                    NodeType::Data
                } else {
                    NodeType::PairList
                }
            })
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

    fn mutable_shape_witness() -> (PrecompileWitness, Arc<AtomicBool>) {
        let data_shape = Arc::new(AtomicBool::new(false));
        let registry = Arc::new(
            PrecompileRegistry::new()
                .with_precompile(MutableShapeFixture { data_shape: Arc::clone(&data_shape) }),
        );
        let mut state = DeferredState::new(registry).unwrap();
        let statement = state
            .register(
                Node::try_pair_list(
                    MutableShapeFixture::tag(),
                    alloc::vec![(TRUE_DIGEST, TRUE_DIGEST)],
                )
                .unwrap(),
            )
            .unwrap();
        state.log_statement(statement).unwrap();
        (PrecompileWitness::new(state).unwrap(), data_shape)
    }

    fn round_trip(proof: &ExecutionProof) {
        let bytes = proof.to_bytes().unwrap();
        let decoded = ExecutionProof::read_from_bytes(&bytes, registry()).unwrap();
        assert_eq!(decoded.to_bytes().unwrap(), bytes);
    }

    #[test]
    fn proof_minimum_serialized_sizes_match_shortest_canonical_encodings() {
        let stark = StarkProof::new(Vec::new(), HashFunction::Blake3_256);
        assert_eq!(StarkProof::min_serialized_size(), stark.to_bytes().len());

        let vm = VmProof::from_parts(stark, TRUE_DIGEST);
        assert_eq!(VmProof::min_serialized_size(), vm.to_bytes().len());

        let precompile = PrecompileProof::from_parts(
            StarkProof::new(Vec::new(), HashFunction::Blake3_256),
            alloc::vec![root(1)],
        )
        .unwrap();
        assert_eq!(PrecompileProof::min_serialized_size(), precompile.to_bytes().len());
        assert_eq!(PrecompileProof::min_serialized_size(), 35);

        let proofs = alloc::vec![precompile.clone(), precompile];
        let bytes = proofs.to_bytes();
        assert_eq!(bytes.len(), 71);
        let decoded =
            Vec::<PrecompileProof>::read_from_bytes_with_budget(&bytes, bytes.len()).unwrap();
        assert_eq!(decoded, proofs);
    }

    #[test]
    fn stark_proof_decoder_rejects_oversized_length_before_payload() {
        let mut bytes = Vec::new();
        bytes.write_usize(MAX_STARK_PROOF_BYTES + 1);

        let error = StarkProof::read_from_bytes(&bytes).unwrap_err();
        let DeserializationError::InvalidValue(message) = error else {
            panic!("expected excessive STARK proof length to be rejected")
        };
        assert!(message.contains("STARK proof contains too many bytes"));
    }

    #[test]
    fn deferred_encoding_propagates_wire_materialization_errors() {
        let (witness, data_shape) = mutable_shape_witness();
        data_shape.store(true, Ordering::Relaxed);

        assert!(matches!(witness.state().to_wire(), Err(IntegrityError::InvalidStructure)));
        assert!(matches!(
            witness.to_bytes(),
            Err(PrecompileWitnessError::Integrity(IntegrityError::InvalidStructure))
        ));

        let proof = ExecutionProof::Deferred {
            vm: vm_proof(witness.root()),
            precompile: witness,
        };
        assert!(matches!(
            proof.to_bytes(),
            Err(ExecutionProofTransportError::Witness(PrecompileWitnessError::Integrity(
                IntegrityError::InvalidStructure
            )))
        ));
    }

    #[test]
    fn precompile_proof_is_singular_checked_and_ordered() {
        let first = root(1);
        let second = root(2);
        let singleton = precompile_proof(&[first]);
        let merged = precompile_proof(&[first, second]);

        assert_eq!(singleton.aggregate_root(), first);
        assert_eq!(merged.aggregate_root(), crate::deferred::fold_deferred_root(first, second));
        assert_eq!(
            PrecompileProof::from_parts(dummy_stark_proof(&[2]), Vec::new()).unwrap_err(),
            ExecutionProofError::EmptyPrecompileRoots
        );
        assert_eq!(
            PrecompileProof::from_parts(
                dummy_stark_proof(&[2]),
                alloc::vec![first, TRUE_DIGEST, second],
            )
            .unwrap_err(),
            ExecutionProofError::SettledPrecompileRoot { index: 1 }
        );
    }

    #[test]
    fn precompile_proof_enforces_root_count_safety_ceiling() {
        let roots = alloc::vec![root(1); MAX_PRECOMPILE_ROOTS];
        assert_eq!(
            PrecompileProof::from_parts(dummy_stark_proof(&[2]), roots)
                .unwrap()
                .roots()
                .len(),
            MAX_PRECOMPILE_ROOTS
        );

        let roots = alloc::vec![root(1); MAX_PRECOMPILE_ROOTS + 1];
        assert_eq!(
            PrecompileProof::from_parts(dummy_stark_proof(&[2]), roots).unwrap_err(),
            ExecutionProofError::TooManyPrecompileRoots {
                roots: MAX_PRECOMPILE_ROOTS + 1,
                max: MAX_PRECOMPILE_ROOTS,
            }
        );

        let mut bytes = dummy_stark_proof(&[2]).to_bytes();
        bytes.write_usize(MAX_PRECOMPILE_ROOTS + 1);
        let error = PrecompileProof::read_from_bytes(&bytes).unwrap_err();
        let DeserializationError::InvalidValue(message) = error else {
            panic!("expected excessive root count to be rejected")
        };
        assert!(message.contains("precompile proof contains too many roots"));
    }

    #[test]
    fn precompile_proof_deserialization_rejects_settled_constituent_root() {
        let malformed = PrecompileProof {
            proof: dummy_stark_proof(&[2]),
            roots: alloc::vec![root(1), TRUE_DIGEST],
        };

        let error = PrecompileProof::read_from_bytes(&malformed.to_bytes()).unwrap_err();
        let DeserializationError::InvalidValue(message) = error else {
            panic!("expected invalid precompile proof shape")
        };
        assert!(message.contains("index 1 is already settled"));
    }

    #[test]
    fn deferred_construction_requires_non_true_matching_singleton_witness() {
        let singleton = witness(1);
        let singleton_root = singleton.root();
        let deferred =
            ExecutionProof::new_deferred(vm_proof(singleton_root), singleton.clone()).unwrap();
        assert!(!deferred.is_complete());
        assert_eq!(deferred.precompile_witness().unwrap().roots(), &[singleton_root]);

        assert_eq!(
            ExecutionProof::new_deferred(vm_proof(TRUE_DIGEST), singleton.clone()).unwrap_err(),
            ExecutionProofError::DeferredTrueRoot
        );
        assert_eq!(
            ExecutionProof::new_deferred(vm_proof(root(99)), singleton).unwrap_err(),
            ExecutionProofError::DeferredRootMismatch
        );

        let merged = PrecompileWitness::merge(alloc::vec![witness(1), witness(2)]).unwrap();
        let merged_root = merged.root();
        assert_eq!(
            ExecutionProof::new_deferred(vm_proof(merged_root), merged).unwrap_err(),
            ExecutionProofError::DeferredNonSingletonWitness { roots: 2 }
        );
    }

    #[test]
    fn complete_construction_enforces_empty_exact_and_larger_coverage() {
        let empty = ExecutionProof::new_complete(vm_proof(TRUE_DIGEST), None).unwrap();
        assert!(empty.is_complete());
        assert!(empty.precompile().is_none());

        assert_eq!(
            ExecutionProof::new_complete(vm_proof(root(1)), None).unwrap_err(),
            ExecutionProofError::MissingPrecompileProof
        );
        assert_eq!(
            ExecutionProof::new_complete(
                vm_proof(TRUE_DIGEST),
                Some(precompile_proof(&[root(1)])),
            )
            .unwrap_err(),
            ExecutionProofError::UnexpectedPrecompileProof
        );

        let exact =
            ExecutionProof::new_complete(vm_proof(root(2)), Some(precompile_proof(&[root(2)])));
        let larger = ExecutionProof::new_complete(
            vm_proof(root(2)),
            Some(precompile_proof(&[root(1), root(2), root(3)])),
        );
        let missing = ExecutionProof::new_complete(
            vm_proof(root(4)),
            Some(precompile_proof(&[root(1), root(2), root(3)])),
        );

        assert!(exact.is_ok());
        assert!(larger.is_ok());
        assert_eq!(missing.unwrap_err(), ExecutionProofError::InsufficientPrecompileRootCoverage);
    }

    #[test]
    fn ordered_coverage_consumes_occurrences_without_deduplicating() {
        let required = [root(1), root(1), root(2)];
        let exact = precompile_proof(&required);
        let larger = precompile_proof(&[root(3), root(1), root(4), root(1), root(2), root(5)]);
        let reordered = precompile_proof(&[root(2), root(1), root(1)]);
        let missing = precompile_proof(&[root(1), root(1)]);
        let multiplicity_losing = precompile_proof(&[root(1), root(2)]);

        assert!(validate_precompile_coverage(required.iter(), Some(&exact)).is_ok());
        assert!(validate_precompile_coverage(required.iter(), Some(&larger)).is_ok());
        for proof in [&reordered, &missing, &multiplicity_losing] {
            assert_eq!(
                validate_precompile_coverage(required.iter(), Some(proof)).unwrap_err(),
                ExecutionProofError::InsufficientPrecompileRootCoverage
            );
        }
    }

    #[test]
    fn deferred_witness_access_is_borrowed_and_cloneable() {
        let retained = witness(1);
        let root = retained.root();
        let proof = ExecutionProof::new_deferred(vm_proof(root), retained).unwrap();
        let owned_witness = proof.precompile_witness().cloned().unwrap();
        assert_eq!(owned_witness.root(), root);
        assert_eq!(proof.vm().precompile_root(), root);

        let complete = proof.complete(precompile_proof(&[root])).unwrap();
        assert!(complete.is_complete());
        assert!(complete.precompile_witness().is_none());
    }

    #[test]
    fn completion_is_checked_and_moves_proof_allocations_on_success() {
        let retained = witness(1);
        let retained_root = retained.root();
        let deferred =
            ExecutionProof::new_deferred(vm_proof(retained_root), retained.clone()).unwrap();
        let precompile = precompile_proof(&[root(9), retained_root]);
        let vm_bytes = deferred.vm().proof().bytes().as_ptr();
        let precompile_bytes = precompile.proof().bytes().as_ptr();
        let precompile_roots = precompile.roots().as_ptr();

        let complete = deferred.complete(precompile).unwrap();

        assert!(complete.is_complete());
        assert_eq!(complete.vm().proof().bytes().as_ptr(), vm_bytes);
        assert_eq!(complete.precompile().unwrap().proof().bytes().as_ptr(), precompile_bytes);
        assert_eq!(complete.precompile().unwrap().roots().as_ptr(), precompile_roots);
        assert_eq!(complete.precompile().unwrap().roots(), &[root(9), retained_root]);

        let deferred = ExecutionProof::new_deferred(vm_proof(retained_root), retained).unwrap();
        assert_eq!(
            deferred.complete(precompile_proof(&[root(9)])).unwrap_err(),
            ExecutionProofError::InsufficientPrecompileRootCoverage
        );

        let complete = ExecutionProof::new_complete(
            vm_proof(retained_root),
            Some(precompile_proof(&[retained_root])),
        )
        .unwrap();
        assert_eq!(
            complete.complete(precompile_proof(&[retained_root])).unwrap_err(),
            ExecutionProofError::AlreadyComplete
        );
    }

    #[test]
    fn all_execution_proof_variants_round_trip_canonically() {
        let retained = witness(1);
        let retained_root = retained.root();
        let deferred = ExecutionProof::new_deferred(vm_proof(retained_root), retained).unwrap();
        let empty = ExecutionProof::new_complete(vm_proof(TRUE_DIGEST), None).unwrap();
        let singleton =
            ExecutionProof::new_complete(vm_proof(root(1)), Some(precompile_proof(&[root(1)])))
                .unwrap();
        let merged = ExecutionProof::new_complete(
            vm_proof(root(2)),
            Some(precompile_proof(&[root(1), root(2), root(3)])),
        )
        .unwrap();

        for proof in [&deferred, &empty, &singleton, &merged] {
            round_trip(proof);
        }
        assert_eq!(
            VmProof::read_from_bytes(&vm_proof(root(3)).to_bytes()).unwrap(),
            vm_proof(root(3))
        );
        let precompile = precompile_proof(&[root(1), root(2)]);
        assert_eq!(PrecompileProof::read_from_bytes(&precompile.to_bytes()).unwrap(), precompile);
    }

    #[test]
    fn execution_proof_transport_preserves_malformed_cross_artifact_shapes() {
        let retained = witness(1);
        let retained_root = retained.root();
        let merged = PrecompileWitness::merge(alloc::vec![witness(1), witness(2)]).unwrap();

        let malformed = [
            ExecutionProof::Deferred {
                vm: vm_proof(TRUE_DIGEST),
                precompile: retained.clone(),
            },
            ExecutionProof::Deferred {
                vm: vm_proof(root(99)),
                precompile: retained,
            },
            ExecutionProof::Deferred {
                vm: vm_proof(merged.root()),
                precompile: merged,
            },
            ExecutionProof::Complete {
                vm: vm_proof(retained_root),
                precompile: None,
            },
            ExecutionProof::Complete {
                vm: vm_proof(TRUE_DIGEST),
                precompile: Some(precompile_proof(&[retained_root])),
            },
            ExecutionProof::Complete {
                vm: vm_proof(root(99)),
                precompile: Some(precompile_proof(&[retained_root])),
            },
        ];

        for proof in malformed {
            let bytes = proof.to_bytes().unwrap();
            let decoded = ExecutionProof::read_from_bytes(&bytes, registry()).unwrap();
            assert_eq!(decoded.to_bytes().unwrap(), bytes);
        }
    }

    #[test]
    fn execution_proof_transport_rejects_bad_discriminants_trailing_bytes_and_bounds() {
        let mut bad_discriminant = alloc::vec![9];
        vm_proof(TRUE_DIGEST).write_into(&mut bad_discriminant);
        assert!(ExecutionProof::read_from_bytes(&bad_discriminant, registry()).is_err());

        let mut trailing = ExecutionProof::new_complete(vm_proof(TRUE_DIGEST), None)
            .unwrap()
            .to_bytes()
            .unwrap();
        trailing.push(0);
        assert!(ExecutionProof::read_from_bytes(&trailing, registry()).is_err());

        let canonical = ExecutionProof::Complete {
            vm: vm_proof(TRUE_DIGEST),
            precompile: None,
        }
        .to_bytes()
        .unwrap();
        assert_eq!(canonical[1], 3, "one STARK byte uses a one-byte vint encoding");
        let mut noncanonical = alloc::vec![canonical[0], 0];
        noncanonical.extend_from_slice(&1u64.to_le_bytes());
        noncanonical.extend_from_slice(&canonical[2..]);
        assert!(matches!(
            ExecutionProof::read_from_bytes(&noncanonical, registry()),
            Err(ExecutionProofTransportError::NonCanonicalEncoding)
        ));

        let mut oversized_proof = Vec::new();
        oversized_proof.write_u8(COMPLETE_PROOF_DISCRIMINANT);
        oversized_proof.write_usize(usize::MAX);
        assert!(ExecutionProof::read_from_bytes(&oversized_proof, registry()).is_err());
    }
}
