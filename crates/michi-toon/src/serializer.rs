//! Direct Serde serializer for streaming Rust structs directly into TOON data structures
//! without intermediate `serde_json::Value` allocations.

use compact_str::CompactString;
use serde::ser::{
    Error as SerError, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant, Serializer,
};
use std::fmt::Display;

use crate::render::Value;

/// Error type for TOON serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToonSerError(pub String);

impl Display for ToonSerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ToonSerError {}

impl SerError for ToonSerError {
    fn custom<T: Display>(msg: T) -> Self {
        Self(msg.to_string())
    }
}

/// Serde serializer that produces a single [`Value`].
pub struct ValueSerializer;

impl Serializer for ValueSerializer {
    type Ok = Value;
    type Error = ToonSerError;

    type SerializeSeq = ImpossibleValue;
    type SerializeTuple = ImpossibleValue;
    type SerializeTupleStruct = ImpossibleValue;
    type SerializeTupleVariant = ImpossibleValue;
    type SerializeMap = ImpossibleValue;
    type SerializeStruct = ImpossibleValue;
    type SerializeStructVariant = ImpossibleValue;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Bool(v))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(i64::from(v)))
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(i64::from(v)))
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(i64::from(v)))
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(v))
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(i64::from(v)))
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(i64::from(v)))
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(i64::from(v)))
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        #[allow(clippy::cast_possible_wrap)]
        Ok(Value::Int(v.try_into().unwrap_or(i64::MAX)))
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Float(f64::from(v)))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Float(v))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        let mut s = CompactString::default();
        s.push(v);
        Ok(Value::Str(s))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Str(CompactString::new(v)))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Str(CompactString::new(format!("{v:?}"))))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_some<T: ?Sized + serde::Serialize>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Str(CompactString::new(variant)))
    }

    fn serialize_newtype_struct<T: ?Sized + serde::Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + serde::Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(SerError::custom("nested sequences are rendered as string fallback"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(SerError::custom("tuples not supported as cell values"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(SerError::custom("tuple structs not supported as cell values"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(SerError::custom("tuple variants not supported as cell values"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(SerError::custom("maps not supported as cell values"))
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        Err(SerError::custom("structs not supported as cell values"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(SerError::custom("struct variants not supported as cell values"))
    }
}

/// Helper type representing impossible composite serialization outcomes for single [`Value`] cells.
pub struct ImpossibleValue;

impl SerializeSeq for ImpossibleValue {
    type Ok = Value;
    type Error = ToonSerError;

    fn serialize_element<T: ?Sized + serde::Serialize>(&mut self, _value: &T) -> Result<(), Self::Error> {
        Err(SerError::custom("unsupported"))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("unsupported"))
    }
}

impl SerializeTuple for ImpossibleValue {
    type Ok = Value;
    type Error = ToonSerError;

    fn serialize_element<T: ?Sized + serde::Serialize>(&mut self, _value: &T) -> Result<(), Self::Error> {
        Err(SerError::custom("unsupported"))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("unsupported"))
    }
}

impl SerializeTupleStruct for ImpossibleValue {
    type Ok = Value;
    type Error = ToonSerError;

    fn serialize_field<T: ?Sized + serde::Serialize>(&mut self, _value: &T) -> Result<(), Self::Error> {
        Err(SerError::custom("unsupported"))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("unsupported"))
    }
}

impl SerializeTupleVariant for ImpossibleValue {
    type Ok = Value;
    type Error = ToonSerError;

    fn serialize_field<T: ?Sized + serde::Serialize>(&mut self, _value: &T) -> Result<(), Self::Error> {
        Err(SerError::custom("unsupported"))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("unsupported"))
    }
}

impl SerializeMap for ImpossibleValue {
    type Ok = Value;
    type Error = ToonSerError;

    fn serialize_key<T: ?Sized + serde::Serialize>(&mut self, _key: &T) -> Result<(), Self::Error> {
        Err(SerError::custom("unsupported"))
    }

    fn serialize_value<T: ?Sized + serde::Serialize>(&mut self, _value: &T) -> Result<(), Self::Error> {
        Err(SerError::custom("unsupported"))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("unsupported"))
    }
}

impl SerializeStruct for ImpossibleValue {
    type Ok = Value;
    type Error = ToonSerError;

    fn serialize_field<T: ?Sized + serde::Serialize>(
        &mut self,
        _key: &'static str,
        _value: &T,
    ) -> Result<(), Self::Error> {
        Err(SerError::custom("unsupported"))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("unsupported"))
    }
}

impl SerializeStructVariant for ImpossibleValue {
    type Ok = Value;
    type Error = ToonSerError;

    fn serialize_field<T: ?Sized + serde::Serialize>(
        &mut self,
        _key: &'static str,
        _value: &T,
    ) -> Result<(), Self::Error> {
        Err(SerError::custom("unsupported"))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("unsupported"))
    }
}
