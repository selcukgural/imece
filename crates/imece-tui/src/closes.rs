//! The outside item a card closes (record 142): the number of an issue
//! of the checkout's own repository, named by the leader with an
//! `assign`, a `post` or a plan card. The number is a model's word, so it
//! is digits and nothing else: no repository, no address, no `#`. Pure:
//! what the argument means, nothing of who closes the item or when.

use crate::model::Item;
use serde_json::Value;

/// The most digits an item's number has: GitHub's are below 2^31.
const DIGITS_MOST: usize = 9;

/// The item `v` (a tool call's `closes`) names: none when it is not
/// given, else a whole number from 1 with at most [`DIGITS_MOST`] digits,
/// as a number or as its digits. Refused with why.
pub fn parse(v: &Value) -> Result<Option<Item>, String> {
    const WHY: &str = "closes must be the number of an issue of this repository, digits only \
                       (58, not #58, a repository's name or an address)";
    let digits = match v {
        Value::Null => return Ok(None),
        Value::Number(n) => n.as_u64().ok_or(WHY)?.to_string(),
        Value::String(s) => s.clone(),
        _ => return Err(WHY.into()),
    };
    let plain = !digits.is_empty()
        && digits.len() <= DIGITS_MOST
        && digits.bytes().all(|b| b.is_ascii_digit())
        && !digits.starts_with('0');
    match (plain, digits.parse::<Item>()) {
        (true, Ok(n)) => Ok(Some(n)),
        _ => Err(WHY.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_item_is_a_whole_number_from_one_in_digits_only() {
        assert_eq!(parse(&Value::Null), Ok(None), "not given");
        assert_eq!(parse(&json!(58)), Ok(Some(58)));
        assert_eq!(parse(&json!(1)), Ok(Some(1)), "the first item");
        assert_eq!(parse(&json!("58")), Ok(Some(58)), "its digits");
        assert_eq!(parse(&json!(999_999_999)), Ok(Some(999_999_999)));
        assert_eq!(parse(&json!("999999999")), Ok(Some(999_999_999)));
        for bad in [
            json!(0),
            json!("0"),
            json!(-58),
            json!(58.5),
            json!(1_000_000_000),
            json!("1000000000"),
            json!("058"),
            json!("#58"),
            json!(" 58"),
            json!("+58"),
            json!("5 8"),
            json!("owner/repo#58"),
            json!("https://github.com/o/r/issues/58"),
            json!("٥٨"),
            json!(""),
            json!([58]),
            json!({ "n": 58 }),
            json!(true),
        ] {
            let why = parse(&bad).expect_err(&bad.to_string());
            assert!(why.starts_with("closes must be the number"), "{bad}: {why}");
        }
    }
}
