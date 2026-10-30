//! Unified bus challenge encoding with per-bus domain separation.
//!
//! Provides [`Challenges`], a single struct for encoding multiset/LogUp bus messages
//! as `bus_prefix[bus] + <beta, message>`. Each bus interaction type gets a unique
//! prefix to ensure domain separation.
//!
//! This type is used by:
//!
//! - **AIR constraints** (symbolic expressions): `Challenges<AB::ExprEF>`
//! - **Processor aux trace builders** (concrete field elements): `Challenges<E>`
//! - **Verifier** (`eval_external`): `Challenges<EF>`
//!
//! See [`super::message`] for the standard coefficient index layout.

#[cfg(not(mirai))]
use alloc::{boxed::Box, vec::Vec};
use core::ops::{AddAssign, Mul};

use miden_crypto::field::PrimeCharacteristicRing;

/// Encodes multiset/LogUp contributions as **bus_prefix\[bus\] + \<beta, message\>**.
///
/// - `alpha`: randomness base (kept public for direct access by lookup encoders)
/// - `beta_powers`: precomputed powers `[beta^0, beta^1, ..., beta^(max_message_width-1)]`
/// - `bus_prefix`: per-bus domain separation constants `bus_prefix[i] = alpha + (i+1) *
///   beta^max_message_width`
///
/// The challenges are derived from permutation randomness:
/// - `alpha = challenges[0]`
/// - `beta  = challenges[1]`
///
/// Widths (`beta_powers.len()` and `bus_prefix.len()`) come from the [`LookupAir`]'s
/// `max_message_width()` / `num_bus_ids()` at construction time. The struct is built
/// once and read-only thereafter — `Box<[EF]>` over `Vec<EF>` drops the unused
/// capacity word and signals fixed length.
///
/// [`LookupAir`]: crate::lookup::LookupAir
pub struct Challenges<EF: PrimeCharacteristicRing> {
    pub alpha: EF,
    #[cfg(not(mirai))]
    pub beta_powers: Box<[EF]>,
    #[cfg(mirai)]
    pub beta_powers: [EF; 16],
    /// Per-bus domain separation: `bus_prefix[i] = alpha + (i+1) * gamma`
    /// where `gamma = beta^max_message_width`.
    #[cfg(not(mirai))]
    pub bus_prefix: Box<[EF]>,
    #[cfg(mirai)]
    pub bus_prefix: [EF; 64],
}

impl<EF: PrimeCharacteristicRing> Challenges<EF> {
    /// Builds `alpha`, precomputed `beta` powers, and per-bus prefixes sized from the
    /// [`LookupAir`]'s `max_message_width()` / `num_bus_ids()`.
    ///
    /// `beta_powers` holds `max_message_width` entries (indices `0..max_message_width`).
    /// `bus_prefix` holds `num_bus_ids` entries.
    /// `gamma = beta^max_message_width` (one power beyond the highest `beta_powers` index).
    ///
    /// [`LookupAir`]: crate::lookup::LookupAir
    pub fn new(alpha: EF, beta: EF, max_message_width: usize, num_bus_ids: usize) -> Self {
        assert!(max_message_width > 0, "max_message_width must be non-zero");
        assert!(num_bus_ids > 0, "num_bus_ids must be non-zero");

        #[cfg(not(mirai))]
        let beta_powers = {
            let mut beta_powers: Vec<EF> = Vec::with_capacity(max_message_width);
            beta_powers.push(EF::ONE);
            for i in 1..max_message_width {
                beta_powers.push(beta_powers[i - 1].clone() * beta.clone());
            }
            beta_powers.into_boxed_slice()
        };

        #[cfg(mirai)]
        let beta_powers = core::array::from_fn(|_| beta.clone());

        // gamma = beta^max_message_width (one power beyond the message range)
        #[cfg(not(mirai))]
        let gamma = beta_powers[max_message_width - 1].clone() * beta;

        #[cfg(not(mirai))]
        let bus_prefix: Box<[EF]> = (0..num_bus_ids)
            .map(|i| alpha.clone() + gamma.clone() * EF::from_u32((i as u32) + 1))
            .collect();

        #[cfg(mirai)]
        let bus_prefix = core::array::from_fn(|_| alpha.clone());
        mirai_annotations::checked_assume!(max_message_width <= beta_powers.len());
        mirai_annotations::checked_assume!(num_bus_ids <= bus_prefix.len());

        let result = Self { alpha, beta_powers, bus_prefix };
        mirai_annotations::postcondition!(max_message_width <= result.beta_powers.len());
        mirai_annotations::postcondition!(num_bus_ids <= result.bus_prefix.len());
        result
    }

    /// Encodes as **bus_prefix\[bus\] + sum(beta_powers\[i\] * elem\[i\])** with K consecutive
    /// elements.
    ///
    /// The `bus` parameter selects the bus interaction type for domain separation.
    #[inline(always)]
    pub fn encode<BF, const K: usize>(&self, bus: usize, elems: [BF; K]) -> EF
    where
        EF: Mul<BF, Output = EF> + AddAssign,
    {
        debug_assert!(
            K <= self.beta_powers.len(),
            "Message length {K} exceeds beta_powers capacity ({})",
            self.beta_powers.len(),
        );
        debug_assert!(
            bus < self.bus_prefix.len(),
            "Bus index {bus} exceeds bus_prefix length ({})",
            self.bus_prefix.len(),
        );
        let mut acc = self.bus_prefix[bus].clone();
        for (i, elem) in elems.into_iter().enumerate() {
            acc += self.beta_powers[i].clone() * elem;
        }
        acc
    }

    /// Returns **sum(beta_powers\[offset + i\] * elems\[i\])**.
    ///
    /// Unlike [`Self::encode`], this does **not** add a bus prefix — callers compose it
    /// with their own prefix and other contributions when a single message absorbs
    /// multiple slices at different β offsets (e.g. addr at β⁰, payload at β²).
    #[inline(always)]
    pub fn inner_product_at<BF: Clone>(&self, offset: usize, elems: &[BF]) -> EF
    where
        EF: Mul<BF, Output = EF> + AddAssign,
    {
        mirai_annotations::checked_assume!(offset <= self.beta_powers.len());
        mirai_annotations::checked_assume!(
            elems.len() <= self.beta_powers.len().saturating_sub(offset)
        );
        debug_assert!(
            elems.len() <= self.beta_powers.len().saturating_sub(offset),
            "inner_product_at range {}..{} exceeds beta_powers length ({})",
            offset,
            offset.saturating_add(elems.len()),
            self.beta_powers.len(),
        );
        let mut acc = EF::ZERO;
        for (beta, elem) in self.beta_powers[offset..].iter().zip(elems) {
            acc += beta.clone() * elem.clone();
        }
        acc
    }

    /// Encodes as **bus_prefix\[bus\] + sum(beta_powers\[layout\[i\]\] * values\[i\])** using
    /// sparse positions.
    ///
    /// The `bus` parameter selects the bus interaction type for domain separation.
    #[inline(always)]
    pub fn encode_sparse<BF, const K: usize>(
        &self,
        bus: usize,
        layout: [usize; K],
        values: [BF; K],
    ) -> EF
    where
        EF: Mul<BF, Output = EF> + AddAssign,
    {
        debug_assert!(
            bus < self.bus_prefix.len(),
            "Bus index {bus} exceeds bus_prefix length ({})",
            self.bus_prefix.len(),
        );
        let mut acc = self.bus_prefix[bus].clone();
        for (idx, value) in layout.into_iter().zip(values) {
            debug_assert!(
                idx < self.beta_powers.len(),
                "encode_sparse index {} exceeds beta_powers length ({})",
                idx,
                self.beta_powers.len()
            );
            acc += self.beta_powers[idx].clone() * value;
        }
        acc
    }
}
