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
