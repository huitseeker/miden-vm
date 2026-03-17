use miden_core::serde::{
    ByteReader, ByteWriter, Deserializable, DeserializationError, Serializable,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{PackageId, TargetType, Word};

/// A package dependency
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(proptest_derive::Arbitrary))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(
    all(feature = "arbitrary", test),
    miden_test_serde_macros::serde_test(binary_serde(true))
)]
pub struct Dependency {
    /// The package id of the dependency.
    pub name: PackageId,
    /// The type of package depended on.
    pub kind: TargetType,
    /// The digest of the dependency.
    ///
    /// Serves as an ultimate source of truth for identifying the dependency version.
    #[cfg_attr(feature = "arbitrary", proptest(value = "Word::default()"))]
    pub digest: Word,
}

impl Dependency {
    /// Returns the dependency name.
    pub fn id(&self) -> &PackageId {
        &self.name
    }
}

impl Serializable for Dependency {
    fn write_into<W: ByteWriter>(&self, target: &mut W) {
        self.name.write_into(target);
        self.kind.write_into(target);
        self.digest.write_into(target);
    }
}

impl Deserializable for Dependency {
    fn read_from<R: ByteReader>(source: &mut R) -> Result<Self, DeserializationError> {
        let name = PackageId::read_from(source)?;
        let kind = TargetType::read_from(source)?;
        let digest = Word::read_from(source)?;
        Ok(Self { name, kind, digest })
    }
}
