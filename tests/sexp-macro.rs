use sexpr::{sexp, Sexp};

#[test]
fn test_boolean() {
    assert_eq!(sexp!(#t), Sexp::from(true));
    assert_eq!(sexp!(#f), Sexp::from(false));
}

#[test]
fn test_keyword() {
    assert_eq!(sexp!(#:keyword), Sexp::new_keyword("keyword"));
}

#[test]
fn test_string() {
    assert_eq!(sexp!("foo"), Sexp::from("foo"));
}

#[test]
fn test_pair() {
    assert_eq!(sexp!(("hello" . "world")), Sexp::new_entry("hello", "world"));
}

#[test]
fn test_list() {
    assert_eq!(sexp!((1 2 "three")), Sexp::from(&[Sexp::from(1), Sexp::from(2), Sexp::from("three")]));
}
