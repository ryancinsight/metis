use super::{Catalog, Locale, MAX_MESSAGE_SLOTS, format_message};

fn de_table() -> Catalog {
    static ENTRIES: &[(&str, &str)] = &[("Open", "Öffnen"), ("Close", "Schließen")];
    Catalog::new(Locale::parse("de").expect("valid test locale"), ENTRIES)
}

#[test]
fn locale_accepts_the_admitted_subset() {
    for tag in [
        "en",
        "de",
        "fr",
        "pt-BR",
        "zh-Hans",
        "es-419",
        "de-CH-1901",
        "abc",
    ] {
        let locale = Locale::parse(tag).unwrap_or_else(|| panic!("must accept {tag}"));
        assert_eq!(locale.tag(), tag);
    }
    assert_eq!(Locale::SOURCE.tag(), "en");
}

#[test]
fn locale_rejects_everything_outside_the_subset() {
    for tag in [
        "",
        "e",
        "engl",
        "e1",
        "en-",
        "-en",
        "en--us",
        "en_us",
        "en!",
        "en-x",
        "en-abcdefghi",
        "a-very-long-tag-that-exceeds-thirty-five-bytes",
        "12",
        "en ",
        " en",
    ] {
        assert!(Locale::parse(tag).is_none(), "must reject {tag:?}");
    }
}

#[test]
fn resolve_hits_translations_and_reports_them() {
    let table = de_table();
    assert_eq!(table.len(), 2);
    assert!(!table.is_empty());
    assert_eq!(table.locale().tag(), "de");
    let hit = table.resolve("Open");
    assert!(hit.is_hit());
    assert_eq!(hit.text(), "Öffnen");
}

#[test]
fn resolve_miss_carries_the_key_itself() {
    let table = de_table();
    let miss = table.resolve("Save");
    assert!(!miss.is_hit());
    assert_eq!(miss.text(), "Save");
}

#[test]
fn catalog_macro_builds_a_working_table() {
    static TABLE: Catalog = catalog!(Locale::SOURCE, {
        "Yes" => "Ja",
        "No" => "Nein",
    });
    assert_eq!(TABLE.resolve("Yes").text(), "Ja");
    assert_eq!(TABLE.resolve("Maybe").text(), "Maybe");
    assert_eq!(TABLE.len(), 2);
}

#[test]
fn empty_table_misses_everything() {
    static EMPTY: &[(&str, &str)] = &[];
    let table = Catalog::new(Locale::SOURCE, EMPTY);
    assert!(table.is_empty());
    assert_eq!(table.resolve("anything").text(), "anything");
}

#[test]
fn format_substitutes_named_slots() {
    let out = format_message("{count} files", &[("count", "3")]);
    assert_eq!(out, "3 files");
}

#[test]
fn format_reorders_and_repeats_slots() {
    let out = format_message("{b} then {a} then {b}", &[("a", "1"), ("b", "2")]);
    assert_eq!(out, "2 then 1 then 2");
}

#[test]
fn format_leaves_unknown_slots_intact() {
    let out = format_message("{known} and {unknown}", &[("known", "yes")]);
    assert_eq!(out, "yes and {unknown}");
}

#[test]
fn format_passes_text_without_slots_through() {
    assert_eq!(format_message("plain", &[("a", "b")]), "plain");
    assert_eq!(format_message("", &[]), "");
}

#[test]
fn format_passes_unbalanced_braces_through() {
    assert_eq!(format_message("a {b", &[]), "a {b");
    assert_eq!(format_message("a }b", &[]), "a }b");
    assert_eq!(format_message("{}", &[]), "{}");
}

#[test]
fn format_bounds_applied_slots() {
    // More slots than MAX_MESSAGE_SLOTS: extras stay intact rather than
    // driving unbounded work.
    let template = (0..MAX_MESSAGE_SLOTS + 4)
        .map(|i| format!("{{s{i}}}"))
        .collect::<Vec<_>>()
        .join(" ");
    let args: Vec<(String, String)> = (0..MAX_MESSAGE_SLOTS + 4)
        .map(|i| (format!("s{i}"), "x".to_string()))
        .collect();
    let refs: Vec<(&str, &str)> = args.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let out = format_message(&template, &refs);
    assert_eq!(
        out.split_whitespace().filter(|w| *w == "x").count(),
        MAX_MESSAGE_SLOTS
    );
}
