//! Compile-time message catalog for UI string localization.
//!
//! Keys are source-language strings and values are their translations, so a
//! missing key fails closed to readable text rather than to a blank or a
//! panic: [`Catalog::resolve`] returns [`Resolved::Hit`] with the translation
//! or [`Resolved::Miss`] carrying the key itself, and [`Resolved::text`]
//! yields either without branching at the call site.
//!
//! Tables are plain `&'static` slices built by [`Catalog::new`], which is
//! `const`, so a catalog costs no startup and no heap: lookup reads the
//! table in place. Lookup is a linear scan, which is the honest cost for an
//! unsorted table; a large production table wants its entries sorted and a
//! binary search, or a match-generated dispatcher, neither of which this
//! proving increment needs.
//!
//! [`Locale`] is validated once at the boundary. Message patterns take named
//! slots (`{name}`), never positional ones, because word order varies by
//! language; see [`format_message`].

/// Longest admitted BCP-47 tag in the implemented subset (see [`Locale`]).
/// Tags are validated by [`Locale::parse`], never assumed.
pub const MAX_LOCALE_TAG_BYTES: usize = 35;
/// Longest admitted message key. Keys are source strings; a longer key is a
/// sentence that should be split, not a lookup.
pub const MAX_KEY_BYTES: usize = 128;
/// Longest admitted translated value.
pub const MAX_VALUE_BYTES: usize = 1024;
/// Most named slots one message pattern may declare. Slots beyond this are
/// a message that should be split, and bounding them keeps [`format_message`]
/// a single pass with no allocation beyond the output.
pub const MAX_MESSAGE_SLOTS: usize = 16;

/// A validated BCP-47 locale tag.
///
/// The admitted subset is `language[-script][-region](-variant)*` where the
/// language is 2–3 ASCII letters and every following subtag is 2–8 ASCII
/// alphanumerics: `en`, `de`, `pt-BR`, `zh-Hans`, `es-419`. This covers every
/// locale the catalog targets without implementing the full BCP-47 grammar
/// (extensions, private use, grandfathered tags), which is rejected rather
/// than partially honored. Stored inline so a locale is `Copy` and never
/// allocates; parse once at the boundary and carry the value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Locale {
    bytes: [u8; MAX_LOCALE_TAG_BYTES],
    len: u8,
}

impl Locale {
    /// The source language the keys are written in. Untranslated catalogs
    /// carry this locale, so a lookup that finds nothing is still a lookup
    /// in a real locale rather than a special case.
    pub const SOURCE: Locale = Locale {
        bytes: [
            b'e', b'n', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
        ],
        len: 2,
    };

    /// Validate `tag` and bind it, or `None` when it is not an admitted tag.
    ///
    /// Rejects empty tags, tags over [`MAX_LOCALE_TAG_BYTES`], non-ASCII,
    /// and any shape outside `language[-script][-region](-variant)*`.
    /// Returns `None` rather than a typed error so locale selection at a
    /// trust boundary stays a validation, with the caller choosing how to
    /// report it; there is no new error-code surface for the proving
    /// increment.
    #[must_use]
    pub fn parse(tag: &str) -> Option<Locale> {
        if tag.is_empty() || tag.len() > MAX_LOCALE_TAG_BYTES {
            return None;
        }
        let mut subtags = tag.split('-');
        // Language: 2–3 ASCII letters. Digits never start a tag.
        let language = subtags.next()?;
        if !(2..=3).contains(&language.len()) || !language.bytes().all(|b| b.is_ascii_alphabetic())
        {
            return None;
        }
        // Later subtags: 2–8 ASCII alphanumerics each (`es-419`,
        // `de-CH-1901`). Single-character singletons are extensions and empty
        // segments are malformed; both are outside the admitted subset.
        for sub in subtags {
            if !(2..=8).contains(&sub.len()) || !sub.bytes().all(|b| b.is_ascii_alphanumeric()) {
                return None;
            }
        }
        let bytes = tag.as_bytes();
        let mut stored = [0u8; MAX_LOCALE_TAG_BYTES];
        stored[..bytes.len()].copy_from_slice(bytes);
        // Checked above against `MAX_LOCALE_TAG_BYTES`, so the fallible
        // conversion cannot fail; `ok()?` keeps it total rather than
        // asserting what the bound already guarantees.
        let len = u8::try_from(bytes.len()).ok()?;
        Some(Locale { bytes: stored, len })
    }

    /// The validated tag text.
    #[must_use]
    pub fn tag(&self) -> &str {
        // `parse` admits ASCII only, so this cannot fail.
        std::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("en")
    }
}

/// The outcome of [`Catalog::resolve`]: the translation, or the key itself
/// when the table does not carry it. Both arms yield text through
/// [`Resolved::text`], so call sites never branch on presence.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resolved<'a> {
    /// The table carries a translation for the key.
    Hit(&'a str),
    /// The table does not carry the key; carries the key itself, which is
    /// the source-language string and therefore always readable. The miss is
    /// visible in the type so the caller can count or log it; it is never
    /// silent and never blank.
    Miss(&'a str),
}

impl<'a> Resolved<'a> {
    /// The text to present, whichever arm resolved.
    #[must_use]
    pub fn text(&self) -> &'a str {
        match self {
            Resolved::Hit(text) | Resolved::Miss(text) => text,
        }
    }

    /// True when the table carried a translation.
    #[must_use]
    pub fn is_hit(&self) -> bool {
        matches!(self, Resolved::Hit(_))
    }
}

/// An immutable key-to-translation table for one [`Locale`].
///
/// Built by [`Catalog::new`], which is `const`, so tables live in static
/// storage: no startup cost, no heap, no I/O on the render path. Lookup is a
/// linear scan over the entries in order, returning the first match; duplicate
/// keys resolve to their first definition, and tables with duplicate keys are
/// a content error the tests reject, not a lookup rule callers must know.
#[derive(Clone, Copy, Debug)]
pub struct Catalog {
    locale: Locale,
    entries: &'static [(&'static str, &'static str)],
}

impl Catalog {
    /// Bind `entries` to `locale`. `const` so tables are built at compile
    /// time into static storage.
    #[must_use]
    pub const fn new(locale: Locale, entries: &'static [(&'static str, &'static str)]) -> Self {
        Catalog { locale, entries }
    }

    /// The locale this table translates into.
    #[must_use]
    pub const fn locale(&self) -> Locale {
        self.locale
    }

    /// Entry count, for capacity assertions in tests.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the table carries no entries.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look `key` up, returning [`Resolved::Hit`] with the translation or
    /// [`Resolved::Miss`] carrying the key itself. The outcome borrows the
    /// key's lifetime, not the table's: a miss carries caller data, and a
    /// hit's `'static` text coerces to any shorter borrow. Never fails,
    /// never allocates, never panics: a missing key is a value, not an error.
    #[must_use]
    pub fn resolve<'k>(&self, key: &'k str) -> Resolved<'k> {
        for (candidate, value) in self.entries {
            if *candidate == key {
                return Resolved::Hit(value);
            }
        }
        Resolved::Miss(key)
    }
}

/// Build a [`Catalog`] from literal entries.
///
/// ```rust
/// use metis_ui_lang::{catalog, catalog::Catalog, catalog::Locale};
///
/// static TABLE: Catalog = catalog!(Locale::SOURCE, {
///     "Open" => "Öffnen",
///     "Close" => "Schließen",
/// });
/// assert!(TABLE.resolve("Open").is_hit());
/// assert_eq!(TABLE.resolve("Open").text(), "Öffnen");
/// ```
///
/// Keys and values are `'static` literals, so the table they form is static
/// storage with no startup and no heap. Duplicate keys resolve to their first
/// definition; do not rely on it.
#[macro_export]
macro_rules! catalog {
    ($locale:expr, { $($key:literal => $value:literal),* $(,)? }) => {
        $crate::catalog::Catalog::new($locale, &[$(($key, $value)),*])
    };
}

/// Render `template` with `{name}` slots filled from `args`.
///
/// Slots are named, never positional, because word order varies by language:
/// `"{count} files"` may need to become `"{count} Dateien"` or a reordering
/// no positional scheme survives. Each `{name}` is replaced by its value;
/// a slot with no matching argument is left intact (fail-closed and visible,
/// so a missing argument reads as a template defect rather than silent data
/// loss); unbalanced braces pass through unchanged. At most
/// [`MAX_MESSAGE_SLOTS`] replacements apply; further slots are left intact
/// so a hostile template cannot drive unbounded work.
#[must_use]
pub fn format_message(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    let mut applied = 0usize;
    while let Some(open) = rest.find('{') {
        let after_open = &rest[open + 1..];
        match after_open.find('}') {
            Some(close) if applied < MAX_MESSAGE_SLOTS => {
                let name = &after_open[..close];
                out.push_str(&rest[..open]);
                match args.iter().find(|(key, _)| *key == name) {
                    Some((_, value)) => {
                        out.push_str(value);
                        applied += 1;
                    }
                    None => {
                        out.push_str(&rest[open..=(open + 1 + close)]);
                    }
                }
                rest = &after_open[close + 1..];
            }
            _ => break,
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
