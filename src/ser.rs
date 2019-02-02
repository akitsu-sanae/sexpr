// Copyright 2017 Zephyr Pellerin
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Serialize a Rust data structure into S-expression data.

use std::fmt;
use std::io;
use std::num::FpCategory;
use std::str;

use super::error::{Error, ErrorCode, Result};
use serde::ser::{self, Impossible};

use dtoa;
use itoa;

/// A structure for serializing Rust values into S-expression.
pub struct Serializer<W, F = CompactFormatter> {
    writer: W,
    formatter: F,
}

impl<W> Serializer<W>
where
    W: io::Write,
{
    /// Creates a new S-expression serializer.
    #[inline]
    pub fn new(writer: W) -> Self {
        Serializer::with_formatter(writer, CompactFormatter)
    }
}

impl<'a, W> Serializer<W, PrettyFormatter<'a>>
where
    W: io::Write,
{
    /// Creates a new S-expression pretty print serializer.
    #[inline]
    pub fn pretty(writer: W) -> Self {
        Serializer::with_formatter(writer, PrettyFormatter::new())
    }
}

impl<W, F> Serializer<W, F>
where
    W: io::Write,
    F: Formatter,
{
    /// Creates a new S-expression visitor whose output will be written to the writer
    /// specified.
    #[inline]
    pub fn with_formatter(writer: W, formatter: F) -> Self {
        Serializer {
            writer: writer,
            formatter: formatter,
        }
    }

    /// Unwrap the `Writer` from the `Serializer`.
    #[inline]
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<'a, W, F> ser::Serializer for &'a mut Serializer<W, F>
where
    W: io::Write,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = Compound<'a, W, F>;
    type SerializeTuple = Compound<'a, W, F>;
    type SerializeTupleStruct = Compound<'a, W, F>;
    type SerializeTupleVariant = Compound<'a, W, F>;
    type SerializeMap = Compound<'a, W, F>;
    type SerializeStruct = Compound<'a, W, F>;
    type SerializeStructVariant = Compound<'a, W, F>;

    #[inline]
    fn serialize_bool(self, value: bool) -> Result<()> {
        try!(self
            .formatter
            .write_bool(&mut self.writer, value)
            .map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn serialize_i8(self, value: i8) -> Result<()> {
        try!(self
            .formatter
            .write_i8(&mut self.writer, value)
            .map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn serialize_i16(self, value: i16) -> Result<()> {
        try!(self
            .formatter
            .write_i16(&mut self.writer, value)
            .map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn serialize_i32(self, value: i32) -> Result<()> {
        try!(self
            .formatter
            .write_i32(&mut self.writer, value)
            .map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn serialize_i64(self, value: i64) -> Result<()> {
        try!(self
            .formatter
            .write_i64(&mut self.writer, value)
            .map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn serialize_u8(self, value: u8) -> Result<()> {
        try!(self
            .formatter
            .write_u8(&mut self.writer, value)
            .map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn serialize_u16(self, value: u16) -> Result<()> {
        try!(self
            .formatter
            .write_u16(&mut self.writer, value)
            .map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn serialize_u32(self, value: u32) -> Result<()> {
        try!(self
            .formatter
            .write_u32(&mut self.writer, value)
            .map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn serialize_u64(self, value: u64) -> Result<()> {
        try!(self
            .formatter
            .write_u64(&mut self.writer, value)
            .map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn serialize_f32(self, value: f32) -> Result<()> {
        match value.classify() {
            FpCategory::Nan | FpCategory::Infinite => {
                try!(self
                    .formatter
                    .write_null(&mut self.writer)
                    .map_err(Error::io));
            }
            _ => {
                try!(self
                    .formatter
                    .write_f32(&mut self.writer, value)
                    .map_err(Error::io));
            }
        }
        Ok(())
    }

    #[inline]
    fn serialize_f64(self, value: f64) -> Result<()> {
        match value.classify() {
            FpCategory::Nan | FpCategory::Infinite => {
                try!(self
                    .formatter
                    .write_null(&mut self.writer)
                    .map_err(Error::io));
            }
            _ => {
                try!(self
                    .formatter
                    .write_f64(&mut self.writer, value)
                    .map_err(Error::io));
            }
        }
        Ok(())
    }

    #[inline]
    fn serialize_char(self, value: char) -> Result<()> {
        try!(format_escaped_char(&mut self.writer, &mut self.formatter, value).map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn serialize_str(self, value: &str) -> Result<()> {
        try!(format_escaped_str(&mut self.writer, &mut self.formatter, value).map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn serialize_bytes(self, value: &[u8]) -> Result<()> {
        use serde::ser::SerializeSeq;
        let mut seq = try!(self.serialize_seq(Some(value.len())));
        for byte in value {
            try!(seq.serialize_element(byte));
        }
        seq.end()
    }

    #[inline]
    fn serialize_unit(self) -> Result<()> {
        try!(self
            .formatter
            .write_null(&mut self.writer)
            .map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.serialize_str(variant)
    }

    /// Serialize newtypes without an object wrapper.
    #[inline]
    fn serialize_newtype_struct<T: ?Sized>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ser::Serialize,
    {
        try!(self
            .formatter
            .write_bare_string(&mut self.writer, value)
            .map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ser::Serialize,
    {
        try!(self
            .formatter
            .begin_object(&mut self.writer)
            .map_err(Error::io));
        try!(self
            .formatter
            .begin_object_key(&mut self.writer, true)
            .map_err(Error::io));
        try!(self.serialize_str(variant));
        try!(self
            .formatter
            .end_object_key(&mut self.writer)
            .map_err(Error::io));
        try!(self
            .formatter
            .begin_object_value(&mut self.writer)
            .map_err(Error::io));
        try!(value.serialize(&mut *self));
        try!(self
            .formatter
            .end_object_value(&mut self.writer)
            .map_err(Error::io));
        try!(self
            .formatter
            .end_object(&mut self.writer)
            .map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn serialize_none(self) -> Result<()> {
        self.serialize_unit()
    }

    #[inline]
    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<()>
    where
        T: ser::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        if len == Some(0) {
            try!(self
                .formatter
                .begin_array(&mut self.writer)
                .map_err(Error::io));
            try!(self
                .formatter
                .end_array(&mut self.writer)
                .map_err(Error::io));
            Ok(Compound {
                ser: self,
                state: State::Empty,
            })
        } else {
            try!(self
                .formatter
                .begin_array(&mut self.writer)
                .map_err(Error::io));
            Ok(Compound {
                ser: self,
                state: State::First,
            })
        }
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(Some(len))
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        try!(self
            .formatter
            .begin_object(&mut self.writer)
            .map_err(Error::io));
        try!(self
            .formatter
            .begin_object_key(&mut self.writer, true)
            .map_err(Error::io));
        try!(self.serialize_str(variant));
        try!(self
            .formatter
            .end_object_key(&mut self.writer)
            .map_err(Error::io));
        try!(self
            .formatter
            .begin_object_value(&mut self.writer)
            .map_err(Error::io));
        self.serialize_seq(Some(len))
    }

    #[inline]
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        if len == Some(0) {
            try!(self
                .formatter
                .begin_object(&mut self.writer)
                .map_err(Error::io));
            try!(self
                .formatter
                .end_object(&mut self.writer)
                .map_err(Error::io));
            Ok(Compound {
                ser: self,
                state: State::Empty,
            })
        } else {
            try!(self
                .formatter
                .begin_object(&mut self.writer)
                .map_err(Error::io));
            Ok(Compound {
                ser: self,
                state: State::First,
            })
        }
    }

    #[inline]
    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        self.serialize_map(Some(len))
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        try!(self
            .formatter
            .begin_object(&mut self.writer)
            .map_err(Error::io));
        try!(self
            .formatter
            .begin_object_key(&mut self.writer, true)
            .map_err(Error::io));
        try!(self.serialize_str(variant));
        try!(self
            .formatter
            .end_object_key(&mut self.writer)
            .map_err(Error::io));
        try!(self
            .formatter
            .begin_object_value(&mut self.writer)
            .map_err(Error::io));
        self.serialize_map(Some(len))
    }

    fn collect_str<T: ?Sized>(self, value: &T) -> Result<Self::Ok>
    where
        T: fmt::Display,
    {
        use std::fmt::Write;

        struct Adapter<'ser, W: 'ser, F: 'ser> {
            writer: &'ser mut W,
            formatter: &'ser mut F,
            error: Option<io::Error>,
        }

        impl<'ser, W, F> Write for Adapter<'ser, W, F>
        where
            W: io::Write,
            F: Formatter,
        {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                assert!(self.error.is_none());
                match format_escaped_str_contents(self.writer, self.formatter, s) {
                    Ok(()) => Ok(()),
                    Err(err) => {
                        self.error = Some(err);
                        Err(fmt::Error)
                    }
                }
            }
        }

        try!(self
            .formatter
            .begin_string(&mut self.writer)
            .map_err(Error::io));
        {
            let mut adapter = Adapter {
                writer: &mut self.writer,
                formatter: &mut self.formatter,
                error: None,
            };
            match write!(adapter, "{}", value) {
                Ok(()) => assert!(adapter.error.is_none()),
                Err(fmt::Error) => {
                    return Err(Error::io(adapter.error.expect("there should be an error")));
                }
            }
        }
        try!(self
            .formatter
            .end_string(&mut self.writer)
            .map_err(Error::io));
        Ok(())
    }
}

#[doc(hidden)]
#[derive(Eq, PartialEq)]
pub enum State {
    Empty,
    First,
    Rest,
}

#[doc(hidden)]
pub struct Compound<'a, W: 'a, F: 'a> {
    ser: &'a mut Serializer<W, F>,
    state: State,
}

impl<'a, W, F> ser::SerializeSeq for Compound<'a, W, F>
where
    W: io::Write,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: ser::Serialize,
    {
        try!(self
            .ser
            .formatter
            .begin_array_value(&mut self.ser.writer, self.state == State::First)
            .map_err(Error::io));
        self.state = State::Rest;
        try!(value.serialize(&mut *self.ser));
        try!(self
            .ser
            .formatter
            .end_array_value(&mut self.ser.writer)
            .map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<()> {
        match self.state {
            State::Empty => {}
            _ => try!(self
                .ser
                .formatter
                .end_array(&mut self.ser.writer)
                .map_err(Error::io)),
        }
        Ok(())
    }
}

impl<'a, W, F> ser::SerializeTuple for Compound<'a, W, F>
where
    W: io::Write,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: ser::Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a, W, F> ser::SerializeTupleStruct for Compound<'a, W, F>
where
    W: io::Write,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: ser::Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a, W, F> ser::SerializeTupleVariant for Compound<'a, W, F>
where
    W: io::Write,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: ser::Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<()> {
        match self.state {
            State::Empty => {}
            _ => try!(self
                .ser
                .formatter
                .end_array(&mut self.ser.writer)
                .map_err(Error::io)),
        }
        try!(self
            .ser
            .formatter
            .end_object_value(&mut self.ser.writer)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .end_object(&mut self.ser.writer)
            .map_err(Error::io));
        Ok(())
    }
}

impl<'a, W, F> ser::SerializeMap for Compound<'a, W, F>
where
    W: io::Write,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<()>
    where
        T: ser::Serialize,
    {
        try!(self
            .ser
            .formatter
            .begin_object_key(&mut self.ser.writer, self.state == State::First)
            .map_err(Error::io));
        self.state = State::Rest;

        try!(key.serialize(MapKeySerializer { ser: self.ser }));

        try!(self
            .ser
            .formatter
            .end_object_key(&mut self.ser.writer)
            .map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: ser::Serialize,
    {
        try!(self
            .ser
            .formatter
            .begin_object_value(&mut self.ser.writer)
            .map_err(Error::io));
        try!(value.serialize(&mut *self.ser));
        try!(self
            .ser
            .formatter
            .end_object_value(&mut self.ser.writer)
            .map_err(Error::io));
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<()> {
        match self.state {
            State::Empty => {}
            _ => try!(self
                .ser
                .formatter
                .end_object(&mut self.ser.writer)
                .map_err(Error::io)),
        }
        Ok(())
    }
}

impl<'a, W, F> ser::SerializeStruct for Compound<'a, W, F>
where
    W: io::Write,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ser::Serialize,
    {
        try!(ser::SerializeMap::serialize_key(self, key));
        ser::SerializeMap::serialize_value(self, value)
    }

    #[inline]
    fn end(self) -> Result<()> {
        ser::SerializeMap::end(self)
    }
}

impl<'a, W, F> ser::SerializeStructVariant for Compound<'a, W, F>
where
    W: io::Write,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ser::Serialize,
    {
        ser::SerializeStruct::serialize_field(self, key, value)
    }

    #[inline]
    fn end(self) -> Result<()> {
        match self.state {
            State::Empty => {}
            _ => try!(self
                .ser
                .formatter
                .end_object(&mut self.ser.writer)
                .map_err(Error::io)),
        }
        try!(self
            .ser
            .formatter
            .end_object_value(&mut self.ser.writer)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .end_object(&mut self.ser.writer)
            .map_err(Error::io));
        Ok(())
    }
}

struct MapKeySerializer<'a, W: 'a, F: 'a> {
    ser: &'a mut Serializer<W, F>,
}

fn key_must_be_a_string() -> Error {
    Error::syntax(ErrorCode::KeyMustBeAString, 0, 0)
}

impl<'a, W, F> ser::Serializer for MapKeySerializer<'a, W, F>
where
    W: io::Write,
    F: Formatter,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_str(self, value: &str) -> Result<()> {
        self.ser.serialize_str(value)
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.ser.serialize_str(variant)
    }

    #[inline]
    fn serialize_newtype_struct<T: ?Sized>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ser::Serialize,
    {
        value.serialize(self)
    }

    type SerializeSeq = Impossible<(), Error>;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Impossible<(), Error>;
    type SerializeStructVariant = Impossible<(), Error>;

    fn serialize_bool(self, _value: bool) -> Result<()> {
        Err(key_must_be_a_string())
    }

    fn serialize_i8(self, value: i8) -> Result<()> {
        try!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .write_i8(&mut self.ser.writer, value)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io));
        Ok(())
    }

    fn serialize_i16(self, value: i16) -> Result<()> {
        try!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .write_i16(&mut self.ser.writer, value)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io));
        Ok(())
    }

    fn serialize_i32(self, value: i32) -> Result<()> {
        try!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .write_i32(&mut self.ser.writer, value)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io));
        Ok(())
    }

    fn serialize_i64(self, value: i64) -> Result<()> {
        try!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .write_i64(&mut self.ser.writer, value)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io));
        Ok(())
    }

    fn serialize_u8(self, value: u8) -> Result<()> {
        try!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .write_u8(&mut self.ser.writer, value)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io));
        Ok(())
    }

    fn serialize_u16(self, value: u16) -> Result<()> {
        try!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .write_u16(&mut self.ser.writer, value)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io));
        Ok(())
    }

    fn serialize_u32(self, value: u32) -> Result<()> {
        try!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .write_u32(&mut self.ser.writer, value)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io));
        Ok(())
    }

    fn serialize_u64(self, value: u64) -> Result<()> {
        try!(self
            .ser
            .formatter
            .begin_string(&mut self.ser.writer)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .write_u64(&mut self.ser.writer, value)
            .map_err(Error::io));
        try!(self
            .ser
            .formatter
            .end_string(&mut self.ser.writer)
            .map_err(Error::io));
        Ok(())
    }

    fn serialize_f32(self, _value: f32) -> Result<()> {
        Err(key_must_be_a_string())
    }

    fn serialize_f64(self, _value: f64) -> Result<()> {
        Err(key_must_be_a_string())
    }

    fn serialize_char(self, _value: char) -> Result<()> {
        Err(key_must_be_a_string())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<()> {
        Err(key_must_be_a_string())
    }

    fn serialize_unit(self) -> Result<()> {
        Err(key_must_be_a_string())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Err(key_must_be_a_string())
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()>
    where
        T: ser::Serialize,
    {
        Err(key_must_be_a_string())
    }

    fn serialize_none(self) -> Result<()> {
        Err(key_must_be_a_string())
    }

    fn serialize_some<T: ?Sized>(self, _value: &T) -> Result<()>
    where
        T: ser::Serialize,
    {
        Err(key_must_be_a_string())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(key_must_be_a_string())
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(key_must_be_a_string())
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(key_must_be_a_string())
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(key_must_be_a_string())
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(key_must_be_a_string())
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Err(key_must_be_a_string())
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(key_must_be_a_string())
    }
}

/// Represents a character escape code in a type-safe manner.
pub enum CharEscape {
    /// An escaped quote `"`
    Quote,
    /// An escaped reverse solidus `\`
    ReverseSolidus,
    /// An escaped solidus `/`
    Solidus,
    /// An escaped backspace character (usually escaped as `\b`)
    Backspace,
    /// An escaped form feed character (usually escaped as `\f`)
    FormFeed,
    /// An escaped line feed character (usually escaped as `\n`)
    LineFeed,
    /// An escaped carriage return character (usually escaped as `\r`)
    CarriageReturn,
    /// An escaped tab character (usually escaped as `\t`)
    Tab,
    /// An escaped ASCII plane control character (usually escaped as
    /// `\u00XX` where `XX` are two hex characters)
    AsciiControl(u8),
}

impl CharEscape {
    #[inline]
    fn from_escape_table(escape: u8, byte: u8) -> CharEscape {
        match escape {
            self::BB => CharEscape::Backspace,
            self::TT => CharEscape::Tab,
            self::NN => CharEscape::LineFeed,
            self::FF => CharEscape::FormFeed,
            self::RR => CharEscape::CarriageReturn,
            self::QU => CharEscape::Quote,
            self::BS => CharEscape::ReverseSolidus,
            self::U => CharEscape::AsciiControl(byte),
            _ => unreachable!(),
        }
    }
}

/// This trait abstracts away serializing the S-expression control characters, which allows the user to
/// optionally pretty print the S-expression output.
pub trait Formatter {
    /// Writes a `null` value to the specified writer.
    #[inline]
    fn write_null<W: ?Sized>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        writer.write_all(b"#nil")
    }

    /// Writes a `true` or `false` value to the specified writer.
    #[inline]
    fn write_bool<W: ?Sized>(&mut self, writer: &mut W, value: bool) -> io::Result<()>
    where
        W: io::Write,
    {
        // XXX - This needs to be configurable
        let s = if value {
            b"#t" as &[u8]
        } else {
            b"#f" as &[u8]
        };
        writer.write_all(s)
    }

    /// Writes an integer value like `-123` to the specified writer.
    #[inline]
    fn write_i8<W: ?Sized>(&mut self, writer: &mut W, value: i8) -> io::Result<()>
    where
        W: io::Write,
    {
        itoa::write(writer, value).map(|_| ())
    }

    /// Writes an integer value like `-123` to the specified writer.
    #[inline]
    fn write_i16<W: ?Sized>(&mut self, writer: &mut W, value: i16) -> io::Result<()>
    where
        W: io::Write,
    {
        itoa::write(writer, value).map(|_| ())
    }

    /// Writes an integer value like `-123` to the specified writer.
    #[inline]
    fn write_i32<W: ?Sized>(&mut self, writer: &mut W, value: i32) -> io::Result<()>
    where
        W: io::Write,
    {
        itoa::write(writer, value).map(|_| ())
    }

    /// Writes an integer value like `-123` to the specified writer.
    #[inline]
    fn write_i64<W: ?Sized>(&mut self, writer: &mut W, value: i64) -> io::Result<()>
    where
        W: io::Write,
    {
        itoa::write(writer, value).map(|_| ())
    }

    /// Writes an integer value like `123` to the specified writer.
    #[inline]
    fn write_u8<W: ?Sized>(&mut self, writer: &mut W, value: u8) -> io::Result<()>
    where
        W: io::Write,
    {
        itoa::write(writer, value).map(|_| ())
    }

    /// Writes an integer value like `123` to the specified writer.
    #[inline]
    fn write_u16<W: ?Sized>(&mut self, writer: &mut W, value: u16) -> io::Result<()>
    where
        W: io::Write,
    {
        itoa::write(writer, value).map(|_| ())
    }

    /// Writes an integer value like `123` to the specified writer.
    #[inline]
    fn write_u32<W: ?Sized>(&mut self, writer: &mut W, value: u32) -> io::Result<()>
    where
        W: io::Write,
    {
        itoa::write(writer, value).map(|_| ())
    }

    /// Writes an integer value like `123` to the specified writer.
    #[inline]
    fn write_u64<W: ?Sized>(&mut self, writer: &mut W, value: u64) -> io::Result<()>
    where
        W: io::Write,
    {
        itoa::write(writer, value).map(|_| ())
    }

    /// Writes a floating point value like `-31.26e+12` to the specified writer.
    #[inline]
    fn write_f32<W: ?Sized>(&mut self, writer: &mut W, value: f32) -> io::Result<()>
    where
        W: io::Write,
    {
        dtoa::write(writer, value).map(|_| ())
    }

    /// Writes a floating point value like `-31.26e+12` to the specified writer.
    #[inline]
    fn write_f64<W: ?Sized>(&mut self, writer: &mut W, value: f64) -> io::Result<()>
    where
        W: io::Write,
    {
        dtoa::write(writer, value).map(|_| ())
    }

    /// Write a string without any enclosing quotes
    #[inline]
    fn write_bare_string<W: ?Sized, T: ?Sized>(
        &mut self,
        writer: &mut W,
        value: &T,
    ) -> io::Result<()>
    where
        W: io::Write,
        T: ser::Serialize,
    {
        let n = to_string(value).unwrap();
        writer.write_all(&n[1..n.len() - 1].as_bytes())
    }

    /// Called before each series of `write_string_fragment` and
    /// `write_char_escape`.  Writes a `"` to the specified writer.
    #[inline]
    fn begin_string<W: ?Sized>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        writer.write_all(b"\"")
    }

    /// Called after each series of `write_string_fragment` and
    /// `write_char_escape`.  Writes a `"` to the specified writer.
    #[inline]
    fn end_string<W: ?Sized>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        writer.write_all(b"\"")
    }

    /// Writes a string fragment that doesn't need any escaping to the
    /// specified writer.
    #[inline]
    fn write_string_fragment<W: ?Sized>(&mut self, writer: &mut W, fragment: &str) -> io::Result<()>
    where
        W: io::Write,
    {
        writer.write_all(fragment.as_bytes())
    }

    /// Writes a character escape code to the specified writer.
    #[inline]
    fn write_char_escape<W: ?Sized>(
        &mut self,
        writer: &mut W,
        char_escape: CharEscape,
    ) -> io::Result<()>
    where
        W: io::Write,
    {
        use self::CharEscape::*;

        let s = match char_escape {
            Quote => b"\\\"",
            ReverseSolidus => b"\\\\",
            Solidus => b"\\/",
            Backspace => b"\\b",
            FormFeed => b"\\f",
            LineFeed => b"\\n",
            CarriageReturn => b"\\r",
            Tab => b"\\t",
            AsciiControl(byte) => {
                static HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";
                let bytes = &[
                    b'\\',
                    b'u',
                    b'0',
                    b'0',
                    HEX_DIGITS[(byte >> 4) as usize],
                    HEX_DIGITS[(byte & 0xF) as usize],
                ];
                return writer.write_all(bytes);
            }
        };

        writer.write_all(s)
    }

    /// Called before every array.  Writes a `(` to the specified
    /// writer.
    #[inline]
    fn begin_array<W: ?Sized>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        writer.write_all(b"(")
    }

    /// Called after every array.  Writes a `)` to the specified
    /// writer.
    #[inline]
    fn end_array<W: ?Sized>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        writer.write_all(b")")
    }

    /// Called before every array value.  Writes a space if needed to
    /// the specified writer.
    #[inline]
    fn begin_array_value<W: ?Sized>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: io::Write,
    {
        if first {
            Ok(())
        } else {
            writer.write_all(b" ")
        }
    }

    /// Called after every array value.
    #[inline]
    fn end_array_value<W: ?Sized>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        Ok(())
    }

    /// Called before every object.  Writes a `(` to the specified
    /// writer.
    #[inline]
    fn begin_object<W: ?Sized>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        writer.write_all(b"(")
    }

    /// Called after every object.  Writes a `)` to the specified
    /// writer.
    #[inline]
    fn end_object<W: ?Sized>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        writer.write_all(b")")
    }

    /// Called before every object key.
    #[inline]
    fn begin_object_key<W: ?Sized>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: io::Write,
    {
        if first {
            Ok(())
        } else {
            writer.write_all(b" ")
        }
    }

    /// Called after every object key.  A `.` should be written to the
    /// specified writer by either this method or
    /// `begin_object_value`.
    #[inline]
    fn end_object_key<W: ?Sized>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        Ok(())
    }

    /// Called before every object value.  A `.` should be written to
    /// the specified writer by either this method or
    /// `end_object_key`.
    #[inline]
    fn begin_object_value<W: ?Sized>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        writer.write_all(b".")
    }

    /// Called after every object value.
    #[inline]
    fn end_object_value<W: ?Sized>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        Ok(())
    }
}

/// This structure compacts a S-expression value with no extra whitespace.
#[derive(Clone, Debug)]
pub struct CompactFormatter;

impl Formatter for CompactFormatter {}

/// This structure pretty prints a S-expression value to make it human readable.
#[derive(Clone, Debug)]
pub struct PrettyFormatter<'a> {
    current_indent: usize,
    has_value: bool,
    indent: &'a [u8],
}

impl<'a> PrettyFormatter<'a> {
    /// Construct a pretty printer formatter that defaults to using two spaces for indentation.
    pub fn new() -> Self {
        PrettyFormatter::with_indent(b"  ")
    }

    /// Construct a pretty printer formatter that uses the `indent` string for indentation.
    pub fn with_indent(indent: &'a [u8]) -> Self {
        PrettyFormatter {
            current_indent: 0,
            has_value: false,
            indent: indent,
        }
    }
}

impl<'a> Default for PrettyFormatter<'a> {
    fn default() -> Self {
        PrettyFormatter::new()
    }
}

impl<'a> Formatter for PrettyFormatter<'a> {
    #[inline]
    fn begin_array<W: ?Sized>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        self.current_indent += 1;
        self.has_value = false;
        writer.write_all(b"(")
    }

    #[inline]
    fn end_array<W: ?Sized>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        self.current_indent -= 1;

        if self.has_value {
            try!(writer.write_all(b"\n"));
            try!(indent(writer, self.current_indent, self.indent));
        }

        writer.write_all(b")")
    }

    #[inline]
    fn begin_array_value<W: ?Sized>(&mut self, writer: &mut W, _first: bool) -> io::Result<()>
    where
        W: io::Write,
    {
        try!(writer.write_all(b"\n"));
        try!(indent(writer, self.current_indent, self.indent));
        Ok(())
    }

    #[inline]
    fn end_array_value<W: ?Sized>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        self.has_value = true;
        Ok(())
    }

    #[inline]
    fn begin_object<W: ?Sized>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        self.current_indent += 1;
        self.has_value = false;
        writer.write_all(b"{")
    }

    #[inline]
    fn end_object<W: ?Sized>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        self.current_indent -= 1;

        if self.has_value {
            try!(writer.write_all(b"\n"));
            try!(indent(writer, self.current_indent, self.indent));
        }

        writer.write_all(b"}")
    }

    #[inline]
    fn begin_object_key<W: ?Sized>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: io::Write,
    {
        if first {
            try!(writer.write_all(b"\n"));
        } else {
            try!(writer.write_all(b",\n"));
        }
        indent(writer, self.current_indent, self.indent)
    }

    #[inline]
    fn begin_object_value<W: ?Sized>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        writer.write_all(b": ")
    }

    #[inline]
    fn end_object_value<W: ?Sized>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: io::Write,
    {
        self.has_value = true;
        Ok(())
    }
}

fn format_escaped_str<W: ?Sized, F: ?Sized>(
    writer: &mut W,
    formatter: &mut F,
    value: &str,
) -> io::Result<()>
where
    W: io::Write,
    F: Formatter,
{
    try!(formatter.begin_string(writer));
    try!(format_escaped_str_contents(writer, formatter, value));
    try!(formatter.end_string(writer));
    Ok(())
}

fn format_escaped_str_contents<W: ?Sized, F: ?Sized>(
    writer: &mut W,
    formatter: &mut F,
    value: &str,
) -> io::Result<()>
where
    W: io::Write,
    F: Formatter,
{
    let bytes = value.as_bytes();

    let mut start = 0;

    for (i, &byte) in bytes.iter().enumerate() {
        let escape = ESCAPE[byte as usize];
        if escape == 0 {
            continue;
        }

        if start < i {
            try!(formatter.write_string_fragment(writer, &value[start..i]));
        }

        let char_escape = CharEscape::from_escape_table(escape, byte);
        try!(formatter.write_char_escape(writer, char_escape));

        start = i + 1;
    }

    if start != bytes.len() {
        try!(formatter.write_string_fragment(writer, &value[start..]));
    }

    Ok(())
}

const BB: u8 = b'b'; // \x08
const TT: u8 = b't'; // \x09
const NN: u8 = b'n'; // \x0A
const FF: u8 = b'f'; // \x0C
const RR: u8 = b'r'; // \x0D
const QU: u8 = b'"'; // \x22
const BS: u8 = b'\\'; // \x5C
const U: u8 = b'u'; // \x00...\x1F except the ones above

// Lookup table of escape sequences. A value of b'x' at index i means that byte
// i is escaped as "\x" in S-expression. A value of 0 means that byte i is not escaped.
#[cfg_attr(rustfmt, rustfmt_skip)]
static ESCAPE: [u8; 256] = [
    //  1   2   3   4   5   6   7   8   9   A   B   C   D   E   F
    U,  U,  U,  U,  U,  U,  U,  U, BB, TT, NN,  U, FF, RR,  U,  U, // 0
    U,  U,  U,  U,  U,  U,  U,  U,  U,  U,  U,  U,  U,  U,  U,  U, // 1
    0,  0, QU,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // 2
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // 3
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // 4
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, BS,  0,  0,  0, // 5
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // 6
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // 7
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // 8
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // 9
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // A
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // B
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // C
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // D
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // E
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // F
];

#[inline]
fn format_escaped_char<W: ?Sized, F: ?Sized>(
    wr: &mut W,
    formatter: &mut F,
    value: char,
) -> io::Result<()>
where
    W: io::Write,
    F: Formatter,
{
    use std::io::Write;
    // A char encoded as UTF-8 takes 4 bytes at most.
    let mut buf = [0; 4];
    write!(&mut buf[..], "{}", value).expect("write char to 4-byte buffer");
    // Writing a char successfully always produce valid UTF-8.
    // Once we do not support Rust <1.15 we will be able to just use
    // the method `char::encode_utf8`.
    // See https://github.com/serde-rs/json/issues/270.
    let slice = unsafe { str::from_utf8_unchecked(&buf[0..value.len_utf8()]) };
    format_escaped_str(wr, formatter, slice)
}

/// Serialize the given data structure as S-expression into the IO stream.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, or if `T` contains a map with non-string keys.
#[inline]
pub fn to_writer<W, T: ?Sized>(writer: W, value: &T) -> Result<()>
where
    W: io::Write,
    T: ser::Serialize,
{
    let mut ser = Serializer::new(writer);
    try!(value.serialize(&mut ser));
    Ok(())
}

/// Serialize the given data structure as pretty-printed S-expression into the IO
/// stream.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, or if `T` contains a map with non-string keys.
#[inline]
pub fn to_writer_pretty<W, T: ?Sized>(writer: W, value: &T) -> Result<()>
where
    W: io::Write,
    T: ser::Serialize,
{
    let mut ser = Serializer::pretty(writer);
    try!(value.serialize(&mut ser));
    Ok(())
}

/// Serialize the given data structure as a S-expression byte vector.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, or if `T` contains a map with non-string keys.
#[inline]
pub fn to_vec<T: ?Sized>(value: &T) -> Result<Vec<u8>>
where
    T: ser::Serialize,
{
    let mut writer = Vec::with_capacity(128);
    try!(to_writer(&mut writer, value));
    Ok(writer)
}

/// Serialize the given data structure as a pretty-printed S-expression byte vector.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, or if `T` contains a map with non-string keys.
#[inline]
pub fn to_vec_pretty<T: ?Sized>(value: &T) -> Result<Vec<u8>>
where
    T: ser::Serialize,
{
    let mut writer = Vec::with_capacity(128);
    try!(to_writer_pretty(&mut writer, value));
    Ok(writer)
}

/// Serialize the given data structure as a String of S-expression.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, or if `T` contains a map with non-string keys.
#[inline]
pub fn to_string<T: ?Sized>(value: &T) -> Result<String>
where
    T: ser::Serialize,
{
    let vec = try!(to_vec(value));
    let string = unsafe {
        // We do not emit invalid UTF-8.
        String::from_utf8_unchecked(vec)
    };
    Ok(string)
}

/// Serialize the given data structure as a pretty-printed String of S-expression.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, or if `T` contains a map with non-string keys.
#[inline]
pub fn to_string_pretty<T: ?Sized>(value: &T) -> Result<String>
where
    T: ser::Serialize,
{
    let vec = try!(to_vec_pretty(value));
    let string = unsafe {
        // We do not emit invalid UTF-8.
        String::from_utf8_unchecked(vec)
    };
    Ok(string)
}

fn indent<W: ?Sized>(wr: &mut W, n: usize, s: &[u8]) -> io::Result<()>
where
    W: io::Write,
{
    for _ in 0..n {
        try!(wr.write_all(s));
    }

    Ok(())
}
