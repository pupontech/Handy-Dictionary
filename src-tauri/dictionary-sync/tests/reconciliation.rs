use handy_dictionary_sync::{decode, encode, merge, reconcile, Choice, Dictionary, Rule};

fn words(entries: &[&str]) -> Dictionary {
    Dictionary {
        words: entries.iter().map(|s| s.to_string()).collect(),
        replacements: vec![],
    }
}
fn rule(value: Option<&str>) -> Dictionary {
    Dictionary {
        words: vec![],
        replacements: value
            .map(|to| {
                vec![Rule {
                    from: "heard".into(),
                    to: to.into(),
                }]
            })
            .unwrap_or_default(),
    }
}

#[test]
fn two_computers_propagate_deletions_without_resurrection() {
    let initial = words(&["keep", "remove"]);
    let after_a = merge(&initial, &words(&["keep", "from-a"]), &initial).unwrap();
    assert_eq!(after_a, words(&["keep", "from-a"]));
    let after_b = merge(&initial, &words(&["keep", "remove", "from-b"]), &after_a).unwrap();
    assert_eq!(after_b, words(&["keep", "from-b", "from-a"]));
    let a_again = merge(&after_a, &after_a, &after_b).unwrap();
    assert!(!a_again.words.contains(&"remove".into()));
    assert!(a_again.words.contains(&"from-a".into()));
    assert!(a_again.words.contains(&"from-b".into()));
}

#[test]
fn edit_versus_delete_is_conflict_and_can_choose_either_side() {
    let (base, local, remote) = (rule(Some("before")), rule(None), rule(Some("after")));
    assert_eq!(
        merge(&base, &local, &remote),
        Err(vec!["rule:heard".into()])
    );
    assert_eq!(
        reconcile(&base, &local, &remote, Some(Choice::Local)).unwrap(),
        local
    );
    assert_eq!(
        reconcile(&base, &local, &remote, Some(Choice::Remote)).unwrap(),
        remote
    );
}

#[test]
fn choosing_local_conflicts_does_not_discard_independent_remote_entries() {
    let mut local = rule(Some("mine"));
    local.words.push("mine".into());
    let mut remote = rule(Some("theirs"));
    remote.words.push("theirs".into());
    let result = reconcile(&rule(Some("before")), &local, &remote, Some(Choice::Local)).unwrap();
    assert_eq!(result.words, vec!["mine", "theirs"]);
    assert_eq!(result.replacements[0].to, "mine");
}

#[test]
fn first_sync_deduplicates_case_insensitively_and_reports_divergent_casing() {
    assert_eq!(
        merge(
            &Dictionary::default(),
            &words(&["Handy"]),
            &words(&["Handy"])
        )
        .unwrap(),
        words(&["Handy"])
    );
    assert_eq!(
        merge(
            &Dictionary::default(),
            &words(&["Handy"]),
            &words(&["HANDY"])
        ),
        Err(vec!["word:handy".into()])
    );
}

#[test]
fn same_concurrent_edit_is_not_a_conflict() {
    assert_eq!(
        merge(
            &rule(Some("before")),
            &rule(Some("after")),
            &rule(Some("after"))
        )
        .unwrap(),
        rule(Some("after"))
    );
}

#[test]
fn local_deletion_of_unchanged_remote_rule_survives() {
    assert_eq!(
        merge(&rule(Some("before")), &rule(None), &rule(Some("before"))).unwrap(),
        rule(None)
    );
}

#[test]
fn export_has_only_dictionary_fields_and_roundtrips() {
    let mut dictionary = words(&["CJ's Studio", "Handy"]);
    dictionary.replacements = rule(Some("after")).replacements;
    let bytes = encode(&dictionary).unwrap();
    assert_eq!(decode(&bytes).unwrap(), dictionary);
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let mut keys: Vec<_> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        vec!["customWords", "replacements", "schema", "version"]
    );
}

#[test]
fn unsupported_or_credential_bearing_documents_are_rejected_whole() {
    let valid: serde_json::Value =
        serde_json::from_slice(&encode(&Dictionary::default()).unwrap()).unwrap();
    for (key, value) in [
        ("version", serde_json::json!(2)),
        ("schema", serde_json::json!("other")),
        ("access_token", serde_json::json!("synthetic-marker")),
    ] {
        let mut document = valid.clone();
        document[key] = value;
        assert!(decode(&serde_json::to_vec(&document).unwrap()).is_err());
    }
}

#[test]
fn malformed_duplicate_and_oversized_dictionaries_are_rejected() {
    for dictionary in [
        words(&["same", "SAME"]),
        words(&[" "]),
        words(&[&"x".repeat(51)]),
        rule(Some("  ")),
    ] {
        assert!(encode(&dictionary).is_err());
    }
    let mut duplicate = rule(Some("one"));
    duplicate.replacements.push(Rule {
        from: "HEARD".into(),
        to: "two".into(),
    });
    assert!(encode(&duplicate).is_err());
    let too_many = Dictionary {
        words: (0..10_001).map(|i| format!("word{i}")).collect(),
        replacements: vec![],
    };
    assert!(encode(&too_many).is_err());
    assert!(decode(&vec![b' '; 1_000_001]).is_err());
}
