//! The checks a card asks for (record 127): names of Lonca's policy checks
//! (`verification.checks`) given with an `assign` or a `post`. The person's
//! yes to the card's change asks Lonca's merge queue to run them too
//! (`lonca approve --also`): only a human signs an approval, so no agent
//! adds or drops one. Once Lonca's `status` says which checks the policy
//! in force defines (`policy_checks`, lonca#203), a name it lacks is
//! refused where it is written, with the names there are and the nearest;
//! while Lonca has not said, the yes is where a name is judged. Pure: what
//! a list of names means, nothing of where it comes from.

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

/// Those of `checks` that the policy's `known` checks lack, in order.
pub fn unknown(checks: &[String], known: &[String]) -> Vec<String> {
    (checks.iter())
        .filter(|c| !known.contains(c))
        .cloned()
        .collect()
}

/// How many edits apart a name may be from the one it is taken to mean.
const NEAR_EDITS: usize = 2;

/// The name of `known` that `name` was most likely meant to be: the
/// fewest edits away, at most [`NEAR_EDITS`] and fewer than `name` has
/// letters (a name of one letter means nothing near); the first of those
/// as near.
pub fn nearest<'a>(name: &str, known: &'a [String]) -> Option<&'a str> {
    let letters = name.chars().count();
    (known.iter())
        .map(|k| (edits(name, k), k.as_str()))
        .filter(|&(d, _)| d <= NEAR_EDITS && d < letters)
        .min_by_key(|&(d, _)| d)
        .map(|(_, k)| k)
}

/// The edits (a letter put in, taken out or changed) that make `a` `b`.
fn edits(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    // The edits from the first letters of `a` read so far to each start of `b`.
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = match ca == cb {
                true => diagonal,
                false => 1 + diagonal.min(above).min(row[j]),
            };
            diagonal = above;
        }
    }
    row[b.len()]
}

/// Refuses those of `checks` that the policy's `known` checks lack, as a
/// leader reads it: which, the names there are and the nearest, so that
/// it is fixed in the same turn.
pub fn against(checks: &[String], known: &[String]) -> Result<(), String> {
    let lacking = unknown(checks, known);
    if lacking.is_empty() {
        return Ok(());
    }
    let quoted: Vec<String> = lacking.iter().map(|n| format!("{n:?}")).collect();
    let what = match lacking.len() {
        1 => format!("{} is not a check of Lonca's policy", quoted[0]),
        _ => format!("{} are not checks of Lonca's policy", quoted.join(", ")),
    };
    if known.is_empty() {
        return Err(format!(
            "{what}, which names none: leave checks out, and say in the body what to run"
        ));
    }
    let mut why = format!("{what}; it has {}.", listed(known));
    let near: Vec<(&String, &str)> = (lacking.iter())
        .filter_map(|n| nearest(n, known).map(|k| (n, k)))
        .collect();
    match near.as_slice() {
        [] => {}
        [(_, k)] if lacking.len() == 1 => why += &format!(" Did you mean {k}?"),
        _ => {
            let each: Vec<String> = (near.iter())
                .map(|(n, k)| format!("{k} for {n:?}"))
                .collect();
            why += &format!(" Did you mean {}?", each.join(", "));
        }
    }
    Err(why)
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

    fn names(n: &[&str]) -> Vec<String> {
        n.iter().map(|c| c.to_string()).collect()
    }

    #[test]
    fn edits_count_each_letter_put_in_taken_out_or_changed() {
        assert_eq!(edits("api", "api"), 0);
        assert_eq!(edits("apii", "api"), 1, "one out");
        assert_eq!(edits("ap", "api"), 1, "one in");
        assert_eq!(edits("apx", "api"), 1, "one changed");
        assert_eq!(edits("aip", "api"), 2, "two letters turned round");
        assert_eq!(edits("kitten", "sitting"), 3);
        assert_eq!(edits("abc", ""), 3);
        assert_eq!(edits("", "abcd"), 4);
        assert_eq!(edits("e2e", "api"), 3);
        assert_eq!(edits("ab", "ba"), 2);
    }

    #[test]
    fn the_nearest_name_is_the_fewest_edits_away_and_only_when_near() {
        let known = names(&["api", "apis", "e2e"]);
        assert_eq!(
            nearest("apii", &known),
            Some("api"),
            "api first of two as near"
        );
        assert_eq!(nearest("aip", &known), Some("api"), "two edits");
        assert_eq!(nearest("e2", &known), Some("e2e"));
        assert_eq!(nearest("apiss", &known), Some("apis"), "the fewer edits");
        assert_eq!(nearest("apixyz", &known), None, "three edits");
        assert_eq!(nearest("x", &names(&["a"])), None, "one letter, one edit");
        assert_eq!(
            nearest("ab", &names(&["xy"])),
            None,
            "as many edits as letters"
        );
        assert_eq!(
            nearest("ab", &names(&["axy"])),
            None,
            "two letters, two edits"
        );
        assert_eq!(nearest("abc", &names(&["axy"])), Some("axy"), "three, two");
        assert_eq!(nearest("api", &[]), None);
    }

    #[test]
    fn a_check_the_policy_lacks_is_refused_with_the_names_it_has_and_the_nearest() {
        let known = names(&["api", "e2e"]);
        assert_eq!(against(&names(&["api"]), &known), Ok(()));
        assert_eq!(against(&[], &known), Ok(()));
        assert_eq!(against(&[], &[]), Ok(()));
        assert_eq!(
            unknown(&names(&["api", "zz", "e2e", "a"]), &known),
            ["zz", "a"]
        );
        assert_eq!(
            against(&names(&["apii"]), &known),
            Err(
                "\"apii\" is not a check of Lonca's policy; it has api, e2e. Did you mean api?"
                    .into()
            )
        );
        assert_eq!(
            against(&names(&["api", "zzzz"]), &known),
            Err("\"zzzz\" is not a check of Lonca's policy; it has api, e2e.".into())
        );
        assert_eq!(
            against(&names(&["apii", "e2", "zzzz"]), &known),
            Err(
                "\"apii\", \"e2\", \"zzzz\" are not checks of Lonca's policy; it has api, \
                 e2e. Did you mean api for \"apii\", e2e for \"e2\"?"
                    .into()
            )
        );
        assert_eq!(
            against(&names(&["zzzz", "e2"]), &known),
            Err(
                "\"zzzz\", \"e2\" are not checks of Lonca's policy; it has api, e2e. Did you \
                 mean e2e for \"e2\"?"
                    .into()
            ),
            "the one near is not the first"
        );
        assert_eq!(
            against(&names(&["api"]), &[]),
            Err(
                "\"api\" is not a check of Lonca's policy, which names none: leave checks \
                 out, and say in the body what to run"
                    .into()
            )
        );
    }
}
