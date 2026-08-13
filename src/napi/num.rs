//! Validating newtypes for the NAPI numeric boundary.
//!
//! Every JavaScript number that crosses into a `#[napi]` export in
//! [`crate::napi`] is read as an `f64` via `napi_get_value_double` (a
//! non-coercing read — `napi_get_value_int32`'s `ToInt32` wraparound is
//! never in the path) and then validated through one of this module's
//! `TryFrom<f64>` impls before any michi code runs. A `TryFrom<f64>`
//! failure becomes a thrown `napi::Error` before the wrapped function body
//! runs, so there is no coercion step left at any call site to reimplement
//! inconsistently. This is the shared kernel for the whole NAPI numeric
//! boundary: `JsRanged` (added in a later commit) is generic over its
//! bounds so a future resilience-domain module can define its own aliases
//! over the same type without redefining it.
#![allow(clippy::as_conversions, clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]

use napi::bindgen_prelude::{sys, FromNapiValue, ToNapiValue, TypeName, ValidateNapiValue};

/// A JavaScript number accepted only when it is finite, has no fractional
/// part, is `>= 0`, and is `<= 9007199254740991`
/// (`Number.MAX_SAFE_INTEGER`). Stores the validated value as `usize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsCount(usize);

impl JsCount {
    /// Returns the validated count.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

impl TryFrom<f64> for JsCount {
    type Error = String;

    fn try_from(v: f64) -> Result<Self, Self::Error> {
        if v.is_finite() && v.fract() == 0.0 && (0.0..=9_007_199_254_740_991.0).contains(&v) {
            Ok(Self(v as usize))
        } else {
            Err(format!("expected a non-negative integer no greater than 9007199254740991, got {v}"))
        }
    }
}

impl TypeName for JsCount {
    fn type_name() -> &'static str {
        "JsCount"
    }

    fn value_type() -> napi::ValueType {
        napi::ValueType::Number
    }
}

impl ValidateNapiValue for JsCount {}

impl FromNapiValue for JsCount {
    unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
        let v = unsafe { f64::from_napi_value(env, napi_val)? };
        Self::try_from(v).map_err(|msg| napi::Error::new(napi::Status::InvalidArg, msg))
    }
}

impl ToNapiValue for JsCount {
    unsafe fn to_napi_value(env: sys::napi_env, val: Self) -> napi::Result<sys::napi_value> {
        unsafe { f64::to_napi_value(env, val.0 as f64) }
    }
}

/// A JavaScript number accepted only when it is finite, has no fractional
/// part, and lies within `[-9007199254740991, 9007199254740991]` — the
/// range over which a JS number and an `i64` agree exactly. Stores `i64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsInt(i64);

impl JsInt {
    /// Returns the validated integer.
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
}

impl TryFrom<f64> for JsInt {
    type Error = String;

    fn try_from(v: f64) -> Result<Self, Self::Error> {
        if v.is_finite() && v.fract() == 0.0 && (-9_007_199_254_740_991.0..=9_007_199_254_740_991.0).contains(&v) {
            Ok(Self(v as i64))
        } else {
            Err(format!("expected an integer in [-9007199254740991, 9007199254740991], got {v}"))
        }
    }
}

impl TypeName for JsInt {
    fn type_name() -> &'static str {
        "JsInt"
    }

    fn value_type() -> napi::ValueType {
        napi::ValueType::Number
    }
}

impl ValidateNapiValue for JsInt {}

impl FromNapiValue for JsInt {
    unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
        let v = unsafe { f64::from_napi_value(env, napi_val)? };
        Self::try_from(v).map_err(|msg| napi::Error::new(napi::Status::InvalidArg, msg))
    }
}

impl ToNapiValue for JsInt {
    unsafe fn to_napi_value(env: sys::napi_env, val: Self) -> napi::Result<sys::napi_value> {
        unsafe { f64::to_napi_value(env, val.0 as f64) }
    }
}

/// The shared kernel type for every bounded-integer position on the NAPI
/// boundary, in either domain. Accepts a JS number only when it is finite,
/// has no fractional part, and satisfies `MIN <= v <= MAX`. Generic and
/// reusable — not specialized to any single domain's bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsRanged<const MIN: i64, const MAX: i64>(i64);

impl<const MIN: i64, const MAX: i64> JsRanged<MIN, MAX> {
    const U8_FITS: () = assert!(MIN >= 0 && MAX <= u8::MAX as i64, "JsRanged bounds do not fit in u8");

    /// Returns the validated integer.
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }

    /// Returns the validated integer narrowed to `u8`. Only valid when
    /// `MIN >= 0 && MAX <= u8::MAX`; instantiating this function with
    /// bounds that don't fit `u8` is a compile error (`E0080`) via
    /// `U8_FITS`.
    // NOT `const fn`: `u8::try_from` cannot be called inside a const fn on
    // this workspace's pinned rustc 1.96 (E0658 — `TryFrom` is not yet a
    // const trait).
    pub fn get_u8(self) -> u8 {
        let () = Self::U8_FITS;
        u8::try_from(self.0).unwrap_or(u8::MAX)
    }
}

impl<const MIN: i64, const MAX: i64> TryFrom<f64> for JsRanged<MIN, MAX> {
    type Error = String;

    fn try_from(v: f64) -> Result<Self, Self::Error> {
        if v.is_finite() && v.fract() == 0.0 && v >= MIN as f64 && v <= MAX as f64 {
            Ok(Self(v as i64))
        } else {
            Err(format!("expected an integer in [{MIN}, {MAX}], got {v}"))
        }
    }
}

impl<const MIN: i64, const MAX: i64> TypeName for JsRanged<MIN, MAX> {
    fn type_name() -> &'static str {
        "JsRanged"
    }

    fn value_type() -> napi::ValueType {
        napi::ValueType::Number
    }
}

impl<const MIN: i64, const MAX: i64> ValidateNapiValue for JsRanged<MIN, MAX> {}

impl<const MIN: i64, const MAX: i64> FromNapiValue for JsRanged<MIN, MAX> {
    unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
        let v = unsafe { f64::from_napi_value(env, napi_val)? };
        Self::try_from(v).map_err(|msg| napi::Error::new(napi::Status::InvalidArg, msg))
    }
}

impl<const MIN: i64, const MAX: i64> ToNapiValue for JsRanged<MIN, MAX> {
    unsafe fn to_napi_value(env: sys::napi_env, val: Self) -> napi::Result<sys::napi_value> {
        unsafe { f64::to_napi_value(env, val.0 as f64) }
    }
}

/// The rendering-domain alias used at `JsToonValue.decimalsVal`. Consumed
/// via `get_u8()`, which compiles because `[0, 20]` fits `u8`.
pub type JsDecimals = JsRanged<0, 20>;

/// A JavaScript number accepted only when `f64::is_finite` holds — `NaN`,
/// `Infinity`, and `-Infinity` are rejected. Stores `f64`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JsFloat(f64);

impl JsFloat {
    /// Returns the validated float.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for JsFloat {
    type Error = String;

    fn try_from(v: f64) -> Result<Self, Self::Error> {
        if v.is_finite() {
            Ok(Self(v))
        } else {
            Err(format!("expected a finite number, got {v}"))
        }
    }
}

impl TypeName for JsFloat {
    fn type_name() -> &'static str {
        "JsFloat"
    }

    fn value_type() -> napi::ValueType {
        napi::ValueType::Number
    }
}

impl ValidateNapiValue for JsFloat {}

impl FromNapiValue for JsFloat {
    unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
        let v = unsafe { f64::from_napi_value(env, napi_val)? };
        Self::try_from(v).map_err(|msg| napi::Error::new(napi::Status::InvalidArg, msg))
    }
}

impl ToNapiValue for JsFloat {
    unsafe fn to_napi_value(env: sys::napi_env, val: Self) -> napi::Result<sys::napi_value> {
        unsafe { f64::to_napi_value(env, val.0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_count_accepts_valid_domain() {
        for (v, expected) in
            [(0.0, 0usize), (1.0, 1), (42.0, 42), (1e6, 1_000_000), (9_007_199_254_740_991.0, 9_007_199_254_740_991)]
        {
            assert_eq!(JsCount::try_from(v).map(JsCount::get), Ok(expected), "input {v}");
        }
    }

    #[test]
    fn js_count_rejects_out_of_domain() {
        for v in [-1.0, -0.5, 0.5, 1.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 9_007_199_254_740_992.0] {
            let err = JsCount::try_from(v).expect_err("out-of-domain value must be rejected");
            assert_eq!(err, format!("expected a non-negative integer no greater than 9007199254740991, got {v}"));
        }
    }

    #[test]
    fn js_int_accepts_safe_integers() {
        for (v, expected) in [
            (0.0, 0i64),
            (-1.0, -1),
            (1.0, 1),
            (-9_007_199_254_740_991.0, -9_007_199_254_740_991),
            (9_007_199_254_740_991.0, 9_007_199_254_740_991),
            (-1_755_000_000_000.0, -1_755_000_000_000),
        ] {
            assert_eq!(JsInt::try_from(v).map(JsInt::get), Ok(expected), "input {v}");
        }
    }

    #[test]
    fn js_int_rejects_fractional_and_unsafe() {
        for v in
            [1.5, -1.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 9_007_199_254_740_992.0, -9_007_199_254_740_992.0]
        {
            let err = JsInt::try_from(v).expect_err("out-of-domain value must be rejected");
            assert_eq!(err, format!("expected an integer in [-9007199254740991, 9007199254740991], got {v}"));
        }
    }

    #[test]
    fn js_count_type_name_is_js_count() {
        assert_eq!(JsCount::type_name(), "JsCount");
    }

    #[test]
    fn js_int_type_name_is_js_int() {
        assert_eq!(JsInt::type_name(), "JsInt");
    }

    #[test]
    fn js_ranged_type_name_is_js_ranged() {
        assert_eq!(JsRanged::<0, 20>::type_name(), "JsRanged");
    }

    #[test]
    fn js_float_type_name_is_js_float() {
        assert_eq!(JsFloat::type_name(), "JsFloat");
    }

    #[test]
    fn js_decimals_accepts_zero_through_twenty() {
        for (v, expected) in [(0.0, 0u8), (6.0, 6), (20.0, 20)] {
            let d = JsDecimals::try_from(v).expect("in-domain decimals value");
            assert_eq!(d.get_u8(), expected, "input {v}");
        }
    }

    #[test]
    fn js_decimals_rejects_out_of_range() {
        for v in [-1.0, 21.0, 6.5, f64::NAN, f64::INFINITY] {
            let err = JsDecimals::try_from(v).expect_err("out-of-range decimals value must be rejected");
            assert_eq!(err, format!("expected an integer in [0, 20], got {v}"));
        }
    }

    #[test]
    fn js_float_accepts_finite_rejects_nan_and_infinity() {
        for v in [0.0, -0.0, 1.5, -1.5, f64::MAX, f64::MIN_POSITIVE] {
            let f = JsFloat::try_from(v).expect("finite value must be accepted");
            assert_eq!(f.get().to_bits(), v.to_bits(), "input {v}");
        }
        for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let err = JsFloat::try_from(v).expect_err("non-finite value must be rejected");
            assert_eq!(err, format!("expected a finite number, got {v}"));
        }
    }

    #[test]
    fn module_denies_cast_lints() {
        let src = include_str!("../napi.rs");
        // Whitespace-stripped: rustfmt wraps this attribute's ~147-char
        // single-line form across multiple lines under this project's
        // pinned `max_width = 120`, so the substring check tolerates
        // rustfmt's line-wrapping while still pinning the exact lint set
        // and order.
        let stripped: String = src.chars().filter(|c| !c.is_whitespace()).collect();
        let expected: String = "#![deny(clippy::as_conversions, clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_possible_wrap, clippy::cast_precision_loss)]"
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(stripped.contains(&expected), "expected the module-level deny attribute for cast lints");
    }

    #[test]
    fn boundary_carries_exactly_one_cast_allow_and_no_residual_coercion() {
        let napi_src = include_str!("../napi.rs");
        assert_eq!(
            napi_src.matches("#[allow(clippy::cast_possible_truncation").count(),
            1,
            "expected exactly one cast-allow attribute (is_retryable_status) to remain in src/napi.rs"
        );
        for residual in [".max(0)", "usize::try_from", ".clamp(0, 20)"] {
            assert!(!napi_src.contains(residual), "residual coercion expression `{residual}` found in src/napi.rs");
        }
    }
}
