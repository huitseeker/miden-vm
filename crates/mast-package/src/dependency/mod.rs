use alloc::string::String;
use core::fmt;

use miden_core::serde::{
    ByteReader, ByteWriter, Deserializable, DeserializationError, Serializable,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::Word;

pub(crate) mod resolver;

/// The name of a dependency
#[derive(Debug, Clone, PartialEq, Eq, derive_more::From)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[cfg_attr(
    all(feature = "arbitrary", test),
    miden_test_serde_macros::serde_test(binary_serde(true))
)]
pub struct DependencyName(String);

impl DependencyName {
    /// Returns the dependency name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for DependencyName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for DependencyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(feature = "arbitrary")]
impl proptest::arbitrary::Arbitrary for DependencyName {
    type Parameters = ();
    type Strategy = proptest::prelude::BoxedStrategy<Self>;
    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::Strategy;

        let chars = proptest::char::range('a', 'z');
        proptest::collection::vec(chars, 4..32)
            .prop_map(|chars| Self(String::from_iter(chars)))
            .no_shrink()  // Pure random strings, no meaningful shrinking pattern
            .boxed()
    }
}

impl Serializable for DependencyName {
    fn write_into<W: ByteWriter>(&self, target: &mut W) {
        self.0.write_into(target);
    }
}

impl Deserializable for DependencyName {
    fn read_from<R: ByteReader>(source: &mut R) -> Result<Self, DeserializationError> {
        let name = String::read_from(source)?;
        Ok(Self(name))
    }
}

/// A package dependency
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(proptest_derive::Arbitrary))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(
    all(feature = "arbitrary", test),
    miden_test_serde_macros::serde_test(binary_serde(true))
)]
pub struct Dependency {
    /// The name of the dependency.
    /// Serves as a human-readable identifier for the dependency and a search hint for the resolver
    pub name: DependencyName,
    /// The digest of the dependency.
    /// Serves as an ultimate source of truth for identifying the dependency.
    #[cfg_attr(feature = "arbitrary", proptest(value = "Word::default()"))]
    pub digest: Word,
}

impl Dependency {
    /// Returns the dependency name.
    pub fn name(&self) -> &DependencyName {
        &self.name
    }
}

impl Serializable for Dependency {
    fn write_into<W: ByteWriter>(&self, target: &mut W) {
        self.name.0.write_into(target);
        self.digest.write_into(target);
    }
}

impl Deserializable for Dependency {
    fn read_from<R: ByteReader>(source: &mut R) -> Result<Self, DeserializationError> {
        let name = DependencyName(String::read_from(source)?);
        let digest = Word::read_from(source)?;
        Ok(Self { name, digest })
    }
}
