//! MIRAI tag analysis for prover-controlled AIR inputs.

use core::{
    iter::{Product, Sum},
    ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
};

use miden_core::field::{Algebra, PrimeCharacteristicRing, QuadFelt};
use miden_stark::air::{AirBuilder, ExtensionBuilder, LiftedAir, PermutationAirBuilder, RowWindow};
use mirai_annotations::{TAG_PROPAGATION_ALL, TagPropagation, TagPropagationSet, add_tag, has_tag};

use super::{Felt, NUM_PUBLIC_VALUES};

const INPUT_TAG_PROPAGATION: TagPropagationSet = TAG_PROPAGATION_ALL
    & !TagPropagation::SubComponent.into_set()
    & !TagPropagation::SuperComponent.into_set();

struct InputTag<const MASK: TagPropagationSet>;

/// AIR expression used only for tag propagation. Its payload records data flow, not a field value.
#[derive(Clone, Copy, Debug, Default)]
struct TaintExpr(u8);

impl Add for TaintExpr {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl AddAssign for TaintExpr {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for TaintExpr {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl SubAssign for TaintExpr {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Mul for TaintExpr {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl MulAssign for TaintExpr {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl Neg for TaintExpr {
    type Output = Self;

    fn neg(self) -> Self::Output {
        self
    }
}

impl Sum for TaintExpr {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, Add::add)
    }
}

impl Product for TaintExpr {
    fn product<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ONE, Mul::mul)
    }
}

impl PrimeCharacteristicRing for TaintExpr {
    type PrimeSubfield = Felt;

    const ZERO: Self = Self(0);
    const ONE: Self = Self(0);
    const TWO: Self = Self(0);
    const NEG_ONE: Self = Self(0);

    fn from_prime_subfield(_value: Self::PrimeSubfield) -> Self {
        Self::ZERO
    }
}

macro_rules! impl_scalar_algebra {
    ($scalar:ty) => {
        impl From<$scalar> for TaintExpr {
            fn from(_value: $scalar) -> Self {
                Self::ZERO
            }
        }

        impl Add<$scalar> for TaintExpr {
            type Output = Self;

            fn add(self, _rhs: $scalar) -> Self::Output {
                self
            }
        }

        impl AddAssign<$scalar> for TaintExpr {
            fn add_assign(&mut self, _rhs: $scalar) {}
        }

        impl Sub<$scalar> for TaintExpr {
            type Output = Self;

            fn sub(self, _rhs: $scalar) -> Self::Output {
                self
            }
        }

        impl SubAssign<$scalar> for TaintExpr {
            fn sub_assign(&mut self, _rhs: $scalar) {}
        }

        impl Mul<$scalar> for TaintExpr {
            type Output = Self;

            fn mul(self, _rhs: $scalar) -> Self::Output {
                self
            }
        }

        impl MulAssign<$scalar> for TaintExpr {
            fn mul_assign(&mut self, _rhs: $scalar) {}
        }

        impl Algebra<$scalar> for TaintExpr {}
    };
}

impl_scalar_algebra!(Felt);

/// Extension-field counterpart of [`TaintExpr`].
#[derive(Clone, Copy, Debug, Default)]
struct TaintExprExt(u8);

macro_rules! impl_taint_expr_ext_binary_op {
    ($trait:ident, $method:ident) => {
        impl $trait for TaintExprExt {
            type Output = Self;

            fn $method(self, rhs: Self) -> Self::Output {
                Self(self.0 | rhs.0)
            }
        }
    };
}

impl_taint_expr_ext_binary_op!(Add, add);
impl_taint_expr_ext_binary_op!(Sub, sub);
impl_taint_expr_ext_binary_op!(Mul, mul);

impl AddAssign for TaintExprExt {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for TaintExprExt {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl MulAssign for TaintExprExt {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl Neg for TaintExprExt {
    type Output = Self;

    fn neg(self) -> Self::Output {
        self
    }
}

impl Sum for TaintExprExt {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, Add::add)
    }
}

impl Product for TaintExprExt {
    fn product<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ONE, Mul::mul)
    }
}

impl PrimeCharacteristicRing for TaintExprExt {
    type PrimeSubfield = Felt;

    const ZERO: Self = Self(0);
    const ONE: Self = Self(0);
    const TWO: Self = Self(0);
    const NEG_ONE: Self = Self(0);

    fn from_prime_subfield(_value: Self::PrimeSubfield) -> Self {
        Self::ZERO
    }
}

macro_rules! impl_taint_expr_algebra {
    ($trait:ident, $assign_trait:ident, $method:ident, $assign_method:ident) => {
        impl $trait<TaintExpr> for TaintExprExt {
            type Output = Self;

            fn $method(self, rhs: TaintExpr) -> Self::Output {
                Self(self.0 | rhs.0)
            }
        }

        impl $assign_trait<TaintExpr> for TaintExprExt {
            fn $assign_method(&mut self, rhs: TaintExpr) {
                *self = self.$method(rhs);
            }
        }
    };
}

impl From<TaintExpr> for TaintExprExt {
    fn from(value: TaintExpr) -> Self {
        Self(value.0)
    }
}

impl_taint_expr_algebra!(Add, AddAssign, add, add_assign);
impl_taint_expr_algebra!(Sub, SubAssign, sub, sub_assign);
impl_taint_expr_algebra!(Mul, MulAssign, mul, mul_assign);
impl Algebra<TaintExpr> for TaintExprExt {}

macro_rules! impl_quad_felt_algebra {
    ($trait:ident, $assign_trait:ident, $method:ident, $assign_method:ident) => {
        impl $trait<QuadFelt> for TaintExprExt {
            type Output = Self;

            fn $method(self, _rhs: QuadFelt) -> Self::Output {
                self
            }
        }

        impl $assign_trait<QuadFelt> for TaintExprExt {
            fn $assign_method(&mut self, _rhs: QuadFelt) {}
        }
    };
}

impl From<QuadFelt> for TaintExprExt {
    fn from(_value: QuadFelt) -> Self {
        Self::ZERO
    }
}

impl_quad_felt_algebra!(Add, AddAssign, add, add_assign);
impl_quad_felt_algebra!(Sub, SubAssign, sub, sub_assign);
impl_quad_felt_algebra!(Mul, MulAssign, mul, mul_assign);
impl Algebra<QuadFelt> for TaintExprExt {}

struct MiraiAirBuilder<'a> {
    main: RowWindow<'a, TaintExpr>,
    preprocessed: RowWindow<'a, TaintExpr>,
    public_values: &'a [TaintExpr],
    periodic_values: &'a [TaintExpr],
    permutation: RowWindow<'a, TaintExprExt>,
    permutation_randomness: &'a [TaintExprExt],
    permutation_values: &'a [TaintExprExt],
    saw_input_tag: bool,
}

impl<'a> AirBuilder for MiraiAirBuilder<'a> {
    type F = Felt;
    type Expr = TaintExpr;
    type Var = TaintExpr;
    type PreprocessedWindow = RowWindow<'a, TaintExpr>;
    type MainWindow = RowWindow<'a, TaintExpr>;
    type PublicVar = TaintExpr;
    type PeriodicVar = TaintExpr;

    fn main(&self) -> Self::MainWindow {
        self.main
    }

    fn preprocessed(&self) -> &Self::PreprocessedWindow {
        &self.preprocessed
    }

    fn is_first_row(&self) -> Self::Expr {
        TaintExpr::ZERO
    }

    fn is_last_row(&self) -> Self::Expr {
        TaintExpr::ZERO
    }

    fn is_transition(&self) -> Self::Expr {
        TaintExpr::ZERO
    }

    #[inline(always)]
    fn assert_zero<I: Into<Self::Expr>>(&mut self, x: I) {
        if has_tag!(&x.into().0, InputTag<INPUT_TAG_PROPAGATION>) {
            self.saw_input_tag = true;
        }
    }

    fn public_values(&self) -> &[Self::PublicVar] {
        self.public_values
    }

    fn periodic_values(&self) -> &[Self::PeriodicVar] {
        self.periodic_values
    }
}

impl ExtensionBuilder for MiraiAirBuilder<'_> {
    type EF = QuadFelt;
    type ExprEF = TaintExprExt;
    type VarEF = TaintExprExt;

    #[inline(always)]
    fn assert_zero_ext<I>(&mut self, x: I)
    where
        I: Into<Self::ExprEF>,
    {
        if has_tag!(&x.into().0, InputTag<INPUT_TAG_PROPAGATION>) {
            self.saw_input_tag = true;
        }
    }
}

impl<'a> PermutationAirBuilder for MiraiAirBuilder<'a> {
    type MP = RowWindow<'a, TaintExprExt>;
    type RandomVar = TaintExprExt;
    type PermutationVar = TaintExprExt;

    fn permutation(&self) -> Self::MP {
        self.permutation
    }

    fn permutation_randomness(&self) -> &[Self::RandomVar] {
        self.permutation_randomness
    }

    fn permutation_values(&self) -> &[Self::PermutationVar] {
        self.permutation_values
    }
}

macro_rules! eval_core {
    ($builder:expr) => {{
        <super::MidenAir as LiftedAir<Felt, QuadFelt>>::eval(&super::MidenAir::Core, $builder);
    }};
}

macro_rules! eval_chiplets {
    ($builder:expr) => {{
        <super::MidenAir as LiftedAir<Felt, QuadFelt>>::eval(&super::MidenAir::Chiplets, $builder);
    }};
}

macro_rules! eval_eidos_compression {
    ($builder:expr) => {{
        <super::MidenAir as LiftedAir<Felt, QuadFelt>>::eval(
            &super::MidenAir::EidosCompression,
            $builder,
        );
    }};
}

macro_rules! eval_and8_lookup {
    ($builder:expr) => {{
        <super::MidenAir as LiftedAir<Felt, QuadFelt>>::eval(
            &super::MidenAir::And8Lookup,
            $builder,
        );
    }};
}

macro_rules! air_tag_verifier {
    (
        $name:ident,
        main_width = $main_width:expr,
        main_indices = [$($main_index:literal => $main_tag:ident),* $(,)?],
        preprocessed_width = $preprocessed_width:expr,
        auxiliary_width = $auxiliary_width:expr,
        auxiliary_indices = [$($auxiliary_index:literal => $auxiliary_tag:ident),* $(,)?],
        permutation_value_indices = [$($permutation_value_index:literal => $permutation_value_tag:ident),* $(,)?],
        periodic_width = $periodic_width:expr,
        eval = $eval:ident
    ) => {
        pub fn $name() {
            const MAIN_WIDTH: usize = $main_width;
            const PREPROCESSED_WIDTH: usize = $preprocessed_width;
            const AUXILIARY_WIDTH: usize = $auxiliary_width;
            const PERIODIC_WIDTH: usize = $periodic_width;
            const NUM_PERMUTATION_VALUES: usize = 1;

            let main_current = [TaintExpr::ZERO; MAIN_WIDTH];
            let main_next = [TaintExpr::ZERO; MAIN_WIDTH];
            let preprocessed_current = [TaintExpr::ZERO; PREPROCESSED_WIDTH];
            let preprocessed_next = [TaintExpr::ZERO; PREPROCESSED_WIDTH];
            let auxiliary_current = [TaintExprExt::ZERO; AUXILIARY_WIDTH];
            let auxiliary_next = [TaintExprExt::ZERO; AUXILIARY_WIDTH];
            let permutation_values = [TaintExprExt::ZERO; NUM_PERMUTATION_VALUES];

            $(
                add_tag!(&main_current[$main_index].0, InputTag<INPUT_TAG_PROPAGATION>);
                add_tag!(&main_next[$main_index].0, InputTag<INPUT_TAG_PROPAGATION>);
            )*
            $(
                add_tag!(
                    &auxiliary_current[$auxiliary_index].0,
                    InputTag<INPUT_TAG_PROPAGATION>
                );
                add_tag!(
                    &auxiliary_next[$auxiliary_index].0,
                    InputTag<INPUT_TAG_PROPAGATION>
                );
            )*
            $(
                add_tag!(
                    &permutation_values[$permutation_value_index].0,
                    InputTag<INPUT_TAG_PROPAGATION>
                );
            )*

            let public_values = [TaintExpr::ZERO; NUM_PUBLIC_VALUES];
            let periodic_values = [TaintExpr::ZERO; PERIODIC_WIDTH];
            let permutation_randomness =
                [TaintExprExt::ZERO; super::trace::AUX_TRACE_RAND_CHALLENGES];
            let mut builder = MiraiAirBuilder {
                main: RowWindow::from_two_rows(&main_current, &main_next),
                preprocessed: RowWindow::from_two_rows(
                    &preprocessed_current,
                    &preprocessed_next,
                ),
                public_values: &public_values,
                periodic_values: &periodic_values,
                permutation: RowWindow::from_two_rows(&auxiliary_current, &auxiliary_next),
                permutation_randomness: &permutation_randomness,
                permutation_values: &permutation_values,
                saw_input_tag: false,
            };

            $eval!(&mut builder);
            mirai_annotations::mirai_verify(
                builder.saw_input_tag,
                concat!(stringify!($name), " input reaches a constraint")
            );
        }
    };
}

macro_rules! core_tag_verifier {
    (
        $name:ident,
        main = [$($main_index:literal => $main_tag:ident),* $(,)?],
        auxiliary = [$($auxiliary_index:literal => $auxiliary_tag:ident),* $(,)?],
        permutation_values = [$($permutation_value_index:literal => $permutation_value_tag:ident),* $(,)?]
    ) => {
        air_tag_verifier!(
            $name,
            main_width = super::constraints::columns::NUM_CORE_COLS,
            main_indices = [$($main_index => $main_tag),*],
            preprocessed_width = 0,
            auxiliary_width = super::constraints::lookup::main_air::MAIN_COLUMN_SHAPE.len(),
            auxiliary_indices = [$($auxiliary_index => $auxiliary_tag),*],
            permutation_value_indices = [
                $($permutation_value_index => $permutation_value_tag),*
            ],
            periodic_width = 0,
            eval = eval_core
        );
    };
}

macro_rules! core_main_tag_verifiers {
    ($($name:ident: $index:literal => $tag:ident),* $(,)?) => {
        $(core_tag_verifier!(
            $name,
            main = [$index => $tag],
            auxiliary = [],
            permutation_values = []
        );)*
    };
}

core_main_tag_verifiers!(
    verify_core_air_main_0: 0 => Main0,
    verify_core_air_main_1: 1 => Main1,
    verify_core_air_main_2: 2 => Main2,
    verify_core_air_main_3: 3 => Main3,
    verify_core_air_main_4: 4 => Main4,
    verify_core_air_main_5: 5 => Main5,
    verify_core_air_main_6: 6 => Main6,
    verify_core_air_main_7: 7 => Main7,
    verify_core_air_main_8: 8 => Main8,
    verify_core_air_main_9: 9 => Main9,
    verify_core_air_main_10: 10 => Main10,
    verify_core_air_main_11: 11 => Main11,
    verify_core_air_main_12: 12 => Main12,
    verify_core_air_main_13: 13 => Main13,
    verify_core_air_main_14: 14 => Main14,
    verify_core_air_main_15: 15 => Main15,
    verify_core_air_main_16: 16 => Main16,
    verify_core_air_main_17: 17 => Main17,
    verify_core_air_main_18: 18 => Main18,
    verify_core_air_main_19: 19 => Main19,
    verify_core_air_main_20: 20 => Main20,
    verify_core_air_main_21: 21 => Main21,
    verify_core_air_main_22: 22 => Main22,
    verify_core_air_main_23: 23 => Main23,
    verify_core_air_main_24: 24 => Main24,
    verify_core_air_main_25: 25 => Main25,
    verify_core_air_main_26: 26 => Main26,
    verify_core_air_main_27: 27 => Main27,
    verify_core_air_main_28: 28 => Main28,
    verify_core_air_main_29: 29 => Main29,
    verify_core_air_main_30: 30 => Main30,
    verify_core_air_main_31: 31 => Main31,
    verify_core_air_main_32: 32 => Main32,
    verify_core_air_main_33: 33 => Main33,
    verify_core_air_main_34: 34 => Main34,
    verify_core_air_main_35: 35 => Main35,
    verify_core_air_main_36: 36 => Main36,
    verify_core_air_main_37: 37 => Main37,
    verify_core_air_main_38: 38 => Main38,
    verify_core_air_main_39: 39 => Main39,
    verify_core_air_main_40: 40 => Main40,
    verify_core_air_main_41: 41 => Main41,
    verify_core_air_main_42: 42 => Main42,
    verify_core_air_main_43: 43 => Main43,
    verify_core_air_main_44: 44 => Main44,
    verify_core_air_main_45: 45 => Main45,
    verify_core_air_main_46: 46 => Main46,
    verify_core_air_main_47: 47 => Main47,
    verify_core_air_main_48: 48 => Main48,
    verify_core_air_main_49: 49 => Main49,
    verify_core_air_main_50: 50 => Main50,
);

core_tag_verifier!(verify_core_air_auxiliary_0, main = [], auxiliary = [0 => Auxiliary0], permutation_values = []);
core_tag_verifier!(verify_core_air_auxiliary_1, main = [], auxiliary = [1 => Auxiliary1], permutation_values = []);
core_tag_verifier!(verify_core_air_auxiliary_2, main = [], auxiliary = [2 => Auxiliary2], permutation_values = []);
core_tag_verifier!(verify_core_air_auxiliary_3, main = [], auxiliary = [3 => Auxiliary3], permutation_values = []);
core_tag_verifier!(verify_core_air_permutation_value_0, main = [], auxiliary = [], permutation_values = [0 => PermutationValue0]);

macro_rules! chiplets_tag_verifier {
    (
        $name:ident,
        main = [$($main_index:literal => $main_tag:ident),* $(,)?],
        auxiliary = [$($auxiliary_index:literal => $auxiliary_tag:ident),* $(,)?],
        permutation_values = [$($permutation_value_index:literal => $permutation_value_tag:ident),* $(,)?]
    ) => {
        air_tag_verifier!(
            $name,
            main_width = super::constraints::columns::NUM_CHIPLETS_COLS,
            main_indices = [$($main_index => $main_tag),*],
            preprocessed_width = 0,
            auxiliary_width = super::constraints::lookup::chiplet_air::CHIPLET_COLUMN_SHAPE.len(),
            auxiliary_indices = [$($auxiliary_index => $auxiliary_tag),*],
            permutation_value_indices = [
                $($permutation_value_index => $permutation_value_tag),*
            ],
            periodic_width = super::constraints::chiplets::columns::NUM_PERIODIC_COLUMNS,
            eval = eval_chiplets
        );
    };
}

macro_rules! chiplets_main_tag_verifiers {
    ($($name:ident: $index:literal => $tag:ident),* $(,)?) => {
        $(chiplets_tag_verifier!(
            $name,
            main = [$index => $tag],
            auxiliary = [],
            permutation_values = []
        );)*
    };
}

chiplets_main_tag_verifiers!(
    verify_chiplets_air_main_0: 0 => Main0,
    verify_chiplets_air_main_1: 1 => Main1,
    verify_chiplets_air_main_2: 2 => Main2,
    verify_chiplets_air_main_3: 3 => Main3,
    verify_chiplets_air_main_4: 4 => Main4,
    verify_chiplets_air_main_5: 5 => Main5,
    verify_chiplets_air_main_6: 6 => Main6,
    verify_chiplets_air_main_7: 7 => Main7,
    verify_chiplets_air_main_8: 8 => Main8,
    verify_chiplets_air_main_9: 9 => Main9,
    verify_chiplets_air_main_10: 10 => Main10,
    verify_chiplets_air_main_11: 11 => Main11,
    verify_chiplets_air_main_12: 12 => Main12,
    verify_chiplets_air_main_13: 13 => Main13,
    verify_chiplets_air_main_14: 14 => Main14,
    verify_chiplets_air_main_15: 15 => Main15,
    verify_chiplets_air_main_16: 16 => Main16,
    verify_chiplets_air_main_17: 17 => Main17,
    verify_chiplets_air_main_18: 18 => Main18,
    verify_chiplets_air_main_19: 19 => Main19,
    verify_chiplets_air_main_20: 20 => Main20,
    verify_chiplets_air_main_21: 21 => Main21,
);

chiplets_tag_verifier!(verify_chiplets_air_auxiliary_0, main = [], auxiliary = [0 => Auxiliary0], permutation_values = []);
chiplets_tag_verifier!(verify_chiplets_air_auxiliary_1, main = [], auxiliary = [1 => Auxiliary1], permutation_values = []);
chiplets_tag_verifier!(verify_chiplets_air_auxiliary_2, main = [], auxiliary = [2 => Auxiliary2], permutation_values = []);
chiplets_tag_verifier!(verify_chiplets_air_permutation_value_0, main = [], auxiliary = [], permutation_values = [0 => PermutationValue0]);

macro_rules! eidos_compression_tag_verifier {
    (
        $name:ident,
        main = [$($main_index:literal => $main_tag:ident),* $(,)?],
        auxiliary = [$($auxiliary_index:literal => $auxiliary_tag:ident),* $(,)?],
        permutation_values = [$($permutation_value_index:literal => $permutation_value_tag:ident),* $(,)?]
    ) => {
        air_tag_verifier!(
            $name,
            main_width = super::constraints::eidos_compression::NUM_COLS,
            main_indices = [$($main_index => $main_tag),*],
            preprocessed_width = 0,
            auxiliary_width = super::constraints::eidos_compression::lookup::EIDOS_COMPRESSION_LOOKUP_COLUMN_SHAPE.len(),
            auxiliary_indices = [$($auxiliary_index => $auxiliary_tag),*],
            permutation_value_indices = [
                $($permutation_value_index => $permutation_value_tag),*
            ],
            periodic_width = crate::shared::eidos::periodic::NUM_PERIODIC_COLUMNS,
            eval = eval_eidos_compression
        );
    };
}

macro_rules! eidos_compression_main_tag_verifiers {
    ($($name:ident: $index:literal => $tag:ident),* $(,)?) => {
        $(eidos_compression_tag_verifier!(
            $name,
            main = [$index => $tag],
            auxiliary = [],
            permutation_values = []
        );)*
    };
}

eidos_compression_main_tag_verifiers!(
    verify_eidos_compression_air_main_0: 0 => Main0,
    verify_eidos_compression_air_main_1: 1 => Main1,
    verify_eidos_compression_air_main_2: 2 => Main2,
    verify_eidos_compression_air_main_3: 3 => Main3,
    verify_eidos_compression_air_main_4: 4 => Main4,
    verify_eidos_compression_air_main_5: 5 => Main5,
    verify_eidos_compression_air_main_6: 6 => Main6,
    verify_eidos_compression_air_main_7: 7 => Main7,
    verify_eidos_compression_air_main_8: 8 => Main8,
    verify_eidos_compression_air_main_9: 9 => Main9,
    verify_eidos_compression_air_main_10: 10 => Main10,
    verify_eidos_compression_air_main_11: 11 => Main11,
    verify_eidos_compression_air_main_12: 12 => Main12,
    verify_eidos_compression_air_main_13: 13 => Main13,
    verify_eidos_compression_air_main_14: 14 => Main14,
    verify_eidos_compression_air_main_15: 15 => Main15,
    verify_eidos_compression_air_main_16: 16 => Main16,
    verify_eidos_compression_air_main_17: 17 => Main17,
    verify_eidos_compression_air_main_18: 18 => Main18,
    verify_eidos_compression_air_main_19: 19 => Main19,
    verify_eidos_compression_air_main_20: 20 => Main20,
    verify_eidos_compression_air_main_21: 21 => Main21,
    verify_eidos_compression_air_main_22: 22 => Main22,
    verify_eidos_compression_air_main_23: 23 => Main23,
    verify_eidos_compression_air_main_24: 24 => Main24,
    verify_eidos_compression_air_main_25: 25 => Main25,
    verify_eidos_compression_air_main_26: 26 => Main26,
    verify_eidos_compression_air_main_27: 27 => Main27,
    verify_eidos_compression_air_main_28: 28 => Main28,
    verify_eidos_compression_air_main_29: 29 => Main29,
    verify_eidos_compression_air_main_30: 30 => Main30,
    verify_eidos_compression_air_main_31: 31 => Main31,
    verify_eidos_compression_air_main_32: 32 => Main32,
    verify_eidos_compression_air_main_33: 33 => Main33,
    verify_eidos_compression_air_main_34: 34 => Main34,
    verify_eidos_compression_air_main_35: 35 => Main35,
    verify_eidos_compression_air_main_36: 36 => Main36,
    verify_eidos_compression_air_main_37: 37 => Main37,
    verify_eidos_compression_air_main_38: 38 => Main38,
    verify_eidos_compression_air_main_39: 39 => Main39,
    verify_eidos_compression_air_main_40: 40 => Main40,
    verify_eidos_compression_air_main_41: 41 => Main41,
    verify_eidos_compression_air_main_42: 42 => Main42,
    verify_eidos_compression_air_main_43: 43 => Main43,
    verify_eidos_compression_air_main_44: 44 => Main44,
    verify_eidos_compression_air_main_45: 45 => Main45,
    verify_eidos_compression_air_main_46: 46 => Main46,
    verify_eidos_compression_air_main_47: 47 => Main47,
    verify_eidos_compression_air_main_48: 48 => Main48,
    verify_eidos_compression_air_main_49: 49 => Main49,
    verify_eidos_compression_air_main_50: 50 => Main50,
    verify_eidos_compression_air_main_51: 51 => Main51,
    verify_eidos_compression_air_main_52: 52 => Main52,
    verify_eidos_compression_air_main_53: 53 => Main53,
    verify_eidos_compression_air_main_54: 54 => Main54,
    verify_eidos_compression_air_main_55: 55 => Main55,
    verify_eidos_compression_air_main_56: 56 => Main56,
    verify_eidos_compression_air_main_57: 57 => Main57,
    verify_eidos_compression_air_main_58: 58 => Main58,
    verify_eidos_compression_air_main_59: 59 => Main59,
    verify_eidos_compression_air_main_60: 60 => Main60,
    verify_eidos_compression_air_main_61: 61 => Main61,
    verify_eidos_compression_air_main_62: 62 => Main62,
    verify_eidos_compression_air_main_63: 63 => Main63,
    verify_eidos_compression_air_main_64: 64 => Main64,
    verify_eidos_compression_air_main_65: 65 => Main65,
    verify_eidos_compression_air_main_66: 66 => Main66,
    verify_eidos_compression_air_main_67: 67 => Main67,
    verify_eidos_compression_air_main_68: 68 => Main68,
    verify_eidos_compression_air_main_69: 69 => Main69,
    verify_eidos_compression_air_main_70: 70 => Main70,
    verify_eidos_compression_air_main_71: 71 => Main71,
    verify_eidos_compression_air_main_72: 72 => Main72,
    verify_eidos_compression_air_main_73: 73 => Main73,
    verify_eidos_compression_air_main_74: 74 => Main74,
    verify_eidos_compression_air_main_75: 75 => Main75,
    verify_eidos_compression_air_main_76: 76 => Main76,
    verify_eidos_compression_air_main_77: 77 => Main77,
    verify_eidos_compression_air_main_78: 78 => Main78,
    verify_eidos_compression_air_main_79: 79 => Main79,
    verify_eidos_compression_air_main_80: 80 => Main80,
    verify_eidos_compression_air_main_81: 81 => Main81,
    verify_eidos_compression_air_main_82: 82 => Main82,
    verify_eidos_compression_air_main_83: 83 => Main83,
    verify_eidos_compression_air_main_84: 84 => Main84,
    verify_eidos_compression_air_main_85: 85 => Main85,
    verify_eidos_compression_air_main_86: 86 => Main86,
    verify_eidos_compression_air_main_87: 87 => Main87,
    verify_eidos_compression_air_main_88: 88 => Main88,
    verify_eidos_compression_air_main_89: 89 => Main89,
    verify_eidos_compression_air_main_90: 90 => Main90,
    verify_eidos_compression_air_main_91: 91 => Main91,
    verify_eidos_compression_air_main_92: 92 => Main92,
    verify_eidos_compression_air_main_93: 93 => Main93,
    verify_eidos_compression_air_main_94: 94 => Main94,
    verify_eidos_compression_air_main_95: 95 => Main95,
    verify_eidos_compression_air_main_96: 96 => Main96,
    verify_eidos_compression_air_main_97: 97 => Main97,
    verify_eidos_compression_air_main_98: 98 => Main98,
    verify_eidos_compression_air_main_99: 99 => Main99,
    verify_eidos_compression_air_main_100: 100 => Main100,
    verify_eidos_compression_air_main_101: 101 => Main101,
    verify_eidos_compression_air_main_102: 102 => Main102,
    verify_eidos_compression_air_main_103: 103 => Main103,
    verify_eidos_compression_air_main_104: 104 => Main104,
    verify_eidos_compression_air_main_105: 105 => Main105,
    verify_eidos_compression_air_main_106: 106 => Main106,
    verify_eidos_compression_air_main_107: 107 => Main107,
);

eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_0, main = [], auxiliary = [0 => Auxiliary0], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_1, main = [], auxiliary = [1 => Auxiliary1], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_2, main = [], auxiliary = [2 => Auxiliary2], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_3, main = [], auxiliary = [3 => Auxiliary3], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_4, main = [], auxiliary = [4 => Auxiliary4], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_5, main = [], auxiliary = [5 => Auxiliary5], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_6, main = [], auxiliary = [6 => Auxiliary6], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_7, main = [], auxiliary = [7 => Auxiliary7], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_8, main = [], auxiliary = [8 => Auxiliary8], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_9, main = [], auxiliary = [9 => Auxiliary9], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_10, main = [], auxiliary = [10 => Auxiliary10], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_11, main = [], auxiliary = [11 => Auxiliary11], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_12, main = [], auxiliary = [12 => Auxiliary12], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_13, main = [], auxiliary = [13 => Auxiliary13], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_14, main = [], auxiliary = [14 => Auxiliary14], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_15, main = [], auxiliary = [15 => Auxiliary15], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_16, main = [], auxiliary = [16 => Auxiliary16], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_17, main = [], auxiliary = [17 => Auxiliary17], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_18, main = [], auxiliary = [18 => Auxiliary18], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_auxiliary_19, main = [], auxiliary = [19 => Auxiliary19], permutation_values = []);
eidos_compression_tag_verifier!(verify_eidos_compression_air_permutation_value_0, main = [], auxiliary = [], permutation_values = [0 => PermutationValue0]);

macro_rules! and8_lookup_tag_verifier {
    (
        $name:ident,
        main = [$($main_index:literal => $main_tag:ident),* $(,)?],
        auxiliary = [$($auxiliary_index:literal => $auxiliary_tag:ident),* $(,)?],
        permutation_values = [$($permutation_value_index:literal => $permutation_value_tag:ident),* $(,)?]
    ) => {
        air_tag_verifier!(
            $name,
            main_width = super::constraints::and8_lookup::columns::NUM_AND8_LOOKUP_COLS,
            main_indices = [$($main_index => $main_tag),*],
            preprocessed_width = super::constraints::and8_lookup::columns::NUM_AND8_LOOKUP_PREPROCESSED_COLS,
            auxiliary_width = super::constraints::lookup::and8_lookup_air::AND8_LOOKUP_COLUMN_SHAPE.len(),
            auxiliary_indices = [$($auxiliary_index => $auxiliary_tag),*],
            permutation_value_indices = [
                $($permutation_value_index => $permutation_value_tag),*
            ],
            periodic_width = 0,
            eval = eval_and8_lookup
        );
    };
}

macro_rules! and8_lookup_main_tag_verifiers {
    ($($name:ident: $index:literal => $tag:ident),* $(,)?) => {
        $(and8_lookup_tag_verifier!(
            $name,
            main = [$index => $tag],
            auxiliary = [],
            permutation_values = []
        );)*
    };
}

and8_lookup_main_tag_verifiers!(
    verify_and8_lookup_air_main_0: 0 => Main0,
    verify_and8_lookup_air_main_1: 1 => Main1,
    verify_and8_lookup_air_main_2: 2 => Main2,
    verify_and8_lookup_air_main_3: 3 => Main3,
    verify_and8_lookup_air_main_4: 4 => Main4,
    verify_and8_lookup_air_main_5: 5 => Main5,
    verify_and8_lookup_air_main_6: 6 => Main6,
);

and8_lookup_tag_verifier!(verify_and8_lookup_air_auxiliary_0, main = [], auxiliary = [0 => Auxiliary0], permutation_values = []);
and8_lookup_tag_verifier!(verify_and8_lookup_air_auxiliary_1, main = [], auxiliary = [1 => Auxiliary1], permutation_values = []);
and8_lookup_tag_verifier!(verify_and8_lookup_air_auxiliary_2, main = [], auxiliary = [2 => Auxiliary2], permutation_values = []);
and8_lookup_tag_verifier!(verify_and8_lookup_air_auxiliary_3, main = [], auxiliary = [3 => Auxiliary3], permutation_values = []);
and8_lookup_tag_verifier!(verify_and8_lookup_air_permutation_value_0, main = [], auxiliary = [], permutation_values = [0 => PermutationValue0]);
