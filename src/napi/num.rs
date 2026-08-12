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
}
