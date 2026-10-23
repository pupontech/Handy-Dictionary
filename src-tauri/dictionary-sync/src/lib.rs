//! Provider-independent dictionary reconciliation and direct GitHub API sync.
pub mod credentials;
pub mod github;
pub mod plan;
pub mod storage;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Dictionary {
    #[serde(rename = "customWords")]
    pub words: Vec<String>,
    pub replacements: Vec<Rule>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub from: String,
    pub to: String,
}

/// Three-way merge. Missing keys are deletions, not an invitation to union them back.
/// The baseline is the last dictionary successfully applied on this device.
pub fn merge(
    base: &Dictionary,
    local: &Dictionary,
    remote: &Dictionary,
) -> Result<Dictionary, Vec<String>> {
    reconcile(base, local, remote, None)
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Choice {
    Local,
    Remote,
}

pub fn reconcile(
    base: &Dictionary,
    local: &Dictionary,
    remote: &Dictionary,
    choice: Option<Choice>,
) -> Result<Dictionary, Vec<String>> {
    use std::collections::{BTreeMap, BTreeSet};
    fn entries(d: &Dictionary) -> BTreeMap<String, (String, String)> {
        d.words
            .iter()
            .map(|w| {
                (
                    format!("word:{}", w.to_lowercase()),
                    (w.clone(), String::new()),
                )
            })
            .chain(d.replacements.iter().map(|r| {
                (
                    format!("rule:{}", r.from.to_lowercase()),
                    (r.from.clone(), r.to.clone()),
                )
            }))
            .collect()
    }
    let (b, l, r) = (entries(base), entries(local), entries(remote));
    let mut keys = BTreeSet::new();
    let mut output = Dictionary::default();
    let mut conflicts = vec![];
    // Preserve local precedence for replacement-rule order; append unseen remote entries.
    for key in local
        .words
        .iter()
        .map(|w| format!("word:{}", w.to_lowercase()))
        .chain(
            local
                .replacements
                .iter()
                .map(|r| format!("rule:{}", r.from.to_lowercase())),
        )
        .chain(
            remote
                .words
                .iter()
                .map(|w| format!("word:{}", w.to_lowercase())),
        )
        .chain(
            remote
                .replacements
                .iter()
                .map(|r| format!("rule:{}", r.from.to_lowercase())),
        )
        .chain(b.keys().cloned())
    {
        if !keys.insert(key.clone()) {
            continue;
        }
        let (bv, lv, rv) = (b.get(&key), l.get(&key), r.get(&key));
        let selected = if lv == rv || rv == bv {
            lv
        } else if lv == bv {
            rv
        } else {
            match choice {
                Some(Choice::Local) => lv,
                Some(Choice::Remote) => rv,
                None => {
                    conflicts.push(key);
                    continue;
                }
            }
        };
        if let Some((from, to)) = selected {
            if key.starts_with("word:") {
                output.words.push(from.clone());
            } else {
                output.replacements.push(Rule {
                    from: from.clone(),
                    to: to.clone(),
                });
            }
        }
    }
    if conflicts.is_empty() {
        Ok(output)
    } else {
        Err(conflicts)
    }
}

pub const MAX_BYTES: usize = 1_000_000;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema: String,
    version: u32,
    #[serde(rename = "customWords")]
    words: Vec<String>,
    replacements: Vec<Rule>,
}

pub fn validate(d: &Dictionary) -> Result<(), String> {
    use std::collections::BTreeSet;
    if d.words.len() + d.replacements.len() > 10_000 {
        return Err("dictionary_too_large".into());
    }
    let mut seen = BTreeSet::new();
    for word in &d.words {
        if word.trim().is_empty()
            || word.encode_utf16().count() > 50
            || word.split_whitespace().collect::<Vec<_>>().join(" ") != *word
            || !seen.insert(word.to_lowercase())
        {
            return Err("invalid_dictionary".into());
        }
    }
    seen.clear();
    for rule in &d.replacements {
        if rule.from.trim().is_empty()
            || rule.to.trim().is_empty()
            || rule.from.split_whitespace().collect::<Vec<_>>().join(" ") != rule.from
            || rule.to.trim() != rule.to
            || !seen.insert(rule.from.to_lowercase())
        {
            return Err("invalid_dictionary".into());
        }
    }
    Ok(())
}

pub fn decode(content: &[u8]) -> Result<Dictionary, String> {
    if content.len() > MAX_BYTES {
        return Err("dictionary_too_large".into());
    }
    let doc: Document = serde_json::from_slice(content).map_err(|_| "invalid_dictionary")?;
    if doc.schema != "handy-dictionary" || doc.version != 1 {
        return Err("unsupported_dictionary".into());
    }
    let dictionary = Dictionary {
        words: doc.words,
        replacements: doc.replacements,
    };
    validate(&dictionary)?;
    Ok(dictionary)
}

pub fn encode(d: &Dictionary) -> Result<Vec<u8>, String> {
    validate(d)?;
    let bytes = serde_json::to_vec_pretty(&Document {
        schema: "handy-dictionary".into(),
        version: 1,
        words: d.words.clone(),
        replacements: d.replacements.clone(),
    })
    .map_err(|_| "invalid_dictionary")?;
    if bytes.len() > MAX_BYTES {
        return Err("dictionary_too_large".into());
    }
    Ok(bytes)
}

/// Unrelated settings writes must not overwrite a dictionary changed during network I/O.
pub fn preserve_dictionary_fields(current: &serde_json::Value, incoming: &mut serde_json::Value) {
    for field in ["custom_words", "dictionary_replacements"] {
        if let Some(value) = current.get(field) {
            incoming[field] = value.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(entries: &[&str]) -> Dictionary {
        Dictionary {
            words: entries.iter().map(|s| s.to_string()).collect(),
            replacements: vec![],
        }
    }
    #[test]
    fn unrelated_setting_save_cannot_restore_stale_dictionary() {
        let current = serde_json::json!({"custom_words":["synced"],"dictionary_replacements":[],"theme":"light"});
        let mut incoming = serde_json::json!({"custom_words":["stale"],"dictionary_replacements":[],"theme":"dark"});
        preserve_dictionary_fields(&current, &mut incoming);
        assert_eq!(incoming["custom_words"], serde_json::json!(["synced"]));
        assert_eq!(incoming["theme"], "dark");
    }
    #[test]
    fn dictionary_without_schema_is_not_accepted() {
        assert!(decode(br#"{"customWords":[],"replacements":[]}"#).is_err());
    }
    #[test]
    fn concurrent_rule_edits_require_resolution() {
        let dict = |to: &str| Dictionary {
            words: vec![],
            replacements: vec![Rule {
                from: "heard".into(),
                to: to.into(),
            }],
        };
        assert_eq!(
            merge(&dict("before"), &dict("mine"), &dict("theirs")),
            Err(vec!["rule:heard".into()])
        );
    }
    #[test]
    fn merge_preserves_remote_deletion_and_independent_local_addition() {
        assert_eq!(
            merge(&words(&["old"]), &words(&["old", "new"]), &words(&[])).unwrap(),
            words(&["new"])
        );
    }
}
