// Copyright 2017 Zephyr Pellerin <zv@nxvr.org>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use serde_derive::{Deserialize, Serialize};

use std::fmt::Debug;

//use serde::de::{self, Deserialize};
use serde::ser;

use sexpr::to_string;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Animal {
    Dog,
    Frog(String, Vec<isize>),
    Cat { age: usize, name: String },
    AntHive(Vec<String>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Inner {
    a: (),
    b: usize,
    c: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Outer {
    inner: Vec<Inner>,
}

fn test_encode_ok<T>(errors: &[(T, &str)])
where
    T: PartialEq + Debug + ser::Serialize,
{
    for &(ref value, out) in errors {
        let out = out.to_string();

        let s = to_string(value).unwrap();
        assert_eq!(s, out);

        // deserializer logic
        // disabled for now (you can tell bcuz there are comments)
        // let v = to_value(&value).unwrap();
        // let s = to_string(&v).unwrap();
        // assert_eq!(s, out);
    }
}

#[test]
fn test_write_u64() {
    let tests = &[(3u64, "3"), (std::u64::MAX, &std::u64::MAX.to_string())];
    test_encode_ok(tests);
}

#[test]
fn test_write_i64() {
    let tests = &[
        (3i64, "3"),
        (-2i64, "-2"),
        (-1234i64, "-1234"),
        (std::i64::MIN, &std::i64::MIN.to_string()),
    ];
    test_encode_ok(tests);
}

#[test]
fn test_write_f64() {
    let tests = &[(3.0, "3.0"), (3.1, "3.1"), (-1.5, "-1.5"), (0.5, "0.5")];
    test_encode_ok(tests);
}

#[test]
fn test_write_str() {
    let tests = &[("", "\"\""), ("foo", "\"foo\"")];
    test_encode_ok(tests);
}

#[test]
fn test_write_bool() {
    let tests = &[(true, "#t"), (false, "#f")];
    test_encode_ok(tests);
}

#[test]
fn test_write_sym() {
    let tests = &[("a", "\"a\"")];
    test_encode_ok(tests);
}

// ///
// /// ```rust
// /// # use sexpr::sexp;
// /// #
// /// # use sexpr::atom::Atom;
// /// # fn main() {
// /// assert!(Atom::Keyword("keyword"), Atom::discriminate("#:keyword"));
// /// assert!(Atom::Symbol("symbol"), Atom::discriminate("symbol"));
// /// assert!(Atom::String("string"), Atom::discriminate(r#""string""#));
// /// # }
// /// ```
