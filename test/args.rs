//! Ported from test/args.test.ts.
use lxagents_github::models::args::{
    Range, maybe_flag, one_of, one_of_or, optional_date, optional_flag, optional_int,
    optional_text, raw_text, required_int, required_raw_text, required_text, text_list,
};
use lxagents_github::utils::errors::Refused;
use serde_json::{Value, json};

use crate::helpers::assert_matches;

#[track_caller]
fn refuses<T: std::fmt::Debug>(result: Result<T, Refused>, pattern: &str) {
    match result {
        Ok(v) => panic!("did not refuse: {v:?}"),
        Err(e) => assert_matches(e.message(), pattern),
    }
}

const NULL: Value = Value::Null;

#[test]
fn optional_text_trims_drops_blanks_and_refuses_a_non_string() {
    assert_eq!(optional_text(&NULL, "x").unwrap(), None);
    assert_eq!(optional_text(&json!("   "), "x").unwrap(), None);
    assert_eq!(
        optional_text(&json!("  hi "), "x").unwrap(),
        Some("hi".into())
    );
    refuses(
        optional_text(&json!(5), "x"),
        r"`x` must be a string, got number\.",
    );
}

#[test]
fn required_text_refuses_a_missing_or_blank_value() {
    assert_eq!(required_text(&json!(" a "), "x").unwrap(), "a");
    for value in [NULL, json!(""), json!("  ")] {
        refuses(
            required_text(&value, "x"),
            r"`x` is required and must be a non-empty string\.",
        );
    }
}

#[test]
fn raw_text_keeps_the_text_exactly_and_checks_only_that_it_is_a_string() {
    assert_eq!(
        raw_text(&json!("  keep\n"), "x").unwrap(),
        Some("  keep\n".into())
    );
    assert_eq!(raw_text(&NULL, "x").unwrap(), None);
    refuses(required_raw_text(&NULL, "x", false), r"`x` is required\.");
    refuses(
        required_raw_text(&json!("  "), "x", false),
        "must not be blank",
    );
    assert_eq!(required_raw_text(&json!(""), "x", true).unwrap(), "");
    refuses(
        raw_text(&json!(3), "x"),
        r"`x` must be a string, got number\.",
    );
}

#[test]
fn optional_flag_falls_back_when_absent_and_refuses_a_non_boolean() {
    assert!(optional_flag(&NULL, "x", true).unwrap());
    assert!(!optional_flag(&NULL, "x", false).unwrap());
    assert!(!optional_flag(&json!(false), "x", true).unwrap());
    refuses(
        optional_flag(&json!("yes"), "x", true),
        r"`x` must be true or false, got string\.",
    );
}

#[test]
fn optional_int_and_required_int_take_whole_numbers_inside_the_range() {
    let any = Range::default();
    assert_eq!(optional_int(&NULL, "n", any).unwrap(), None);
    assert_eq!(
        optional_int(&json!(5), "n", Range::between(1, 10)).unwrap(),
        Some(5)
    );
    refuses(
        optional_int(&json!(1.5), "n", any),
        r"`n` must be a whole number, got 1\.5\.",
    );
    refuses(
        optional_int(&json!("3"), "n", any),
        r#"must be a whole number, got "3""#,
    );
    refuses(
        optional_int(&json!(0), "n", any),
        "`n` must be between 1 and",
    );
    refuses(
        optional_int(&json!(11), "n", Range::max(10)),
        r"`n` must be between 1 and 10, got 11\.",
    );
    assert_eq!(required_int(&json!(7), "n", any).unwrap(), 7);
    refuses(
        required_int(&NULL, "n", any),
        r"`n` is required and must be a whole number\.",
    );
}

#[test]
fn one_of_returns_the_fallback_when_absent_and_refuses_a_word_outside_the_set() {
    assert_eq!(one_of_or(&NULL, "m", &["a", "b"], "a").unwrap(), "a");
    assert_eq!(
        one_of(&json!(" b "), "m", &["a", "b"]).unwrap(),
        Some("b".into())
    );
    assert_eq!(one_of(&NULL, "m", &["a", "b"]).unwrap(), None);
    refuses(
        one_of(&json!("c"), "m", &["a", "b"]),
        r"`m` must be one of a, b, got 'c'\.",
    );
}

#[test]
fn text_list_trims_drops_blanks_and_duplicates_and_enforces_its_bounds() {
    let max = usize::MAX;
    assert_eq!(
        text_list(&json!([" a ", "b", "a", "  "]), "l", false, max).unwrap(),
        Some(vec!["a".to_string(), "b".to_string()])
    );
    assert_eq!(text_list(&NULL, "l", false, max).unwrap(), None);
    refuses(text_list(&NULL, "l", true, max), "`l` is required");
    refuses(text_list(&json!(["  "]), "l", true, max), "`l` is required");
    refuses(
        text_list(&json!("a"), "l", false, max),
        r"`l` must be a list of strings\.",
    );
    refuses(
        text_list(&json!([1]), "l", false, max),
        r"`l` must be a list of strings\.",
    );
    refuses(
        text_list(&json!(["a", "b", "c"]), "l", false, 2),
        "at most 2 values, got 3",
    );
}

#[test]
fn maybe_flag_keeps_not_given_apart_from_false() {
    assert_eq!(maybe_flag(&NULL, "x").unwrap(), None);
    assert_eq!(maybe_flag(&json!(false), "x").unwrap(), Some(false));
    assert_eq!(maybe_flag(&json!(true), "x").unwrap(), Some(true));
    refuses(
        maybe_flag(&json!("no"), "x"),
        r"`x` must be true or false, got string\.",
    );
}

#[test]
fn optional_date_takes_an_iso_8601_date_or_timestamp_and_refuses_anything_else() {
    assert_eq!(optional_date(&NULL, "since").unwrap(), None);
    assert_eq!(optional_date(&json!("  "), "since").unwrap(), None);
    assert_eq!(
        optional_date(&json!("2026-10-01"), "since").unwrap(),
        Some("2026-10-01".into())
    );
    assert_eq!(
        optional_date(&json!(" 2026-10-01T12:00:00Z "), "since").unwrap(),
        Some("2026-10-01T12:00:00Z".into())
    );
    for bad in ["yesterday", "10/01/2026", "2026-13-45", "2026-1-1"] {
        refuses(
            optional_date(&json!(bad), "since"),
            "`since` must be an ISO 8601 date",
        );
    }
    refuses(
        optional_date(&json!(5), "since"),
        "`since` must be a string",
    );
}
