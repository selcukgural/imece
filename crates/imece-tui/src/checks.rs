//! The checks a card asks for (record 127): names of Lonca's policy checks
//! (`verification.checks`) given with an `assign` or a `post`. The person's
//! yes to the card's change asks Lonca's merge queue to run them too
//! (`lonca approve --also`): only a human signs an approval, so no agent
//! adds or drops one. Whether the policy defines a name is Lonca's to say
//! at the yes. Pure: what a list of names means, nothing of where it comes
//! from.

use imece_agent::lonca::{CHECKS_MOST, is_check_name};
use serde_json::Value;

/// The checks `v` (a tool call's `checks`) names: nothing when it is not
/// given, else an array of at most [`CHECKS_MOST`] strings, each a name
/// Lonca takes for a check, none twice; sorted, as Lonca takes them.
/// Refused whole with why.
pub fn parse(v: &Value) -> Result<Vec<String>, String> {
    if v.is_null() {
        return Ok(Vec::new());
    }
    let Some(list) = v.as_array() else {
        return Err("checks must be an array of strings".into());
    };
    if list.len() > CHECKS_MOST {
        return Err(format!("a card asks for at most {CHECKS_MOST} checks"));
    }
    let mut out: Vec<String> = Vec::new();
    for item in list {
        let Some(name) = item.as_str() else {
            return Err("checks must be an array of strings".into());
        };
        let name = name.trim();
        if !is_check_name(name) {
            return Err(format!(
                "{name:?} is not a check's name: a-z, 0-9, _ and -, at most 32, and not one \
                 of Lonca's own (build, test, tests, extended, syntax, scope, claim_diff, \
                 merge, test_integrity), which run anyway"
            ));
        }
        if out.iter().any(|o| o == name) {
            return Err(format!("checks lists {name:?} twice"));
        }
        out.push(name.to_string());
    }
    out.sort();
    Ok(out)
}

/// The names as a list is read: `api, e2e`.
pub fn listed(checks: &[String]) -> String {
    checks.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn checks_are_lonca_s_names_sorted_and_none_twice() {
        assert_eq!(parse(&Value::Null), Ok(Vec::new()));
        assert_eq!(parse(&json!([])), Ok(Vec::new()));
        assert_eq!(
            parse(&json!([" e2e ", "api"])),
            Ok(vec!["api".to_string(), "e2e".to_string()])
        );
        let not_strings = Err("checks must be an array of strings".to_string());
        assert_eq!(parse(&json!("api")), not_strings);
        assert_eq!(parse(&json!(["api", 3])), not_strings);
        assert_eq!(
            parse(&json!(["api", "api"])),
            Err("checks lists \"api\" twice".into())
        );
        for bad in ["", "Api", "a b", "build", "tests"] {
            let why = parse(&json!([bad])).unwrap_err();
            assert!(
                why.starts_with(&format!("{bad:?} is not a check's name")),
                "{why}"
            );
        }
        let most: Vec<String> = (0..CHECKS_MOST).map(|n| format!("c{n:02}")).collect();
        assert_eq!(parse(&json!(most)), Ok(most.clone()));
        let more: Vec<String> = (0..=CHECKS_MOST).map(|n| format!("c{n:02}")).collect();
        assert_eq!(
            parse(&json!(more)),
            Err("a card asks for at most 32 checks".into())
        );
        assert_eq!(listed(&most[..2]), "c00, c01");
        assert_eq!(listed(&[]), "");
    }
}
