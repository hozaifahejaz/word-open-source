//! Unicode casing with one output per original scalar, retaining run ownership.
use unicode_segmentation::UnicodeSegmentation;

/// Locale-independent Unicode case mappings. Title capitalizes the first
/// original grapheme of each whitespace-delimited token; Sentence capitalizes
/// the first grapheme and those after `.`, `?`, or `!` followed by whitespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextCase {
    Upper,
    Lower,
    Title,
    Sentence,
}

// Lower the whole segment so contextual Greek sigma works across run boundaries.
fn lower_mappings(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut offset = 0;
    text.chars()
        .map(|ch| {
            let len = ch.to_lowercase().map(char::len_utf8).sum::<usize>();
            let mapped = lower[offset..offset + len].to_owned();
            offset += len;
            mapped
        })
        .collect()
}

pub(crate) fn case_mappings(text: &str, case: TextCase) -> Vec<String> {
    match case {
        TextCase::Upper => text.chars().map(|ch| ch.to_uppercase().collect()).collect(),
        TextCase::Lower => lower_mappings(text),
        TextCase::Title => {
            let mut mappings = Vec::new();
            for token in text.split_inclusive(char::is_whitespace) {
                let first = token.graphemes(true).next().unwrap();
                mappings.extend(
                    first
                        .chars()
                        .map(|ch| ch.to_uppercase().collect::<String>()),
                );
                mappings.extend(lower_mappings(&token[first.len()..]));
            }
            mappings
        }
        TextCase::Sentence => {
            let mut mappings = lower_mappings(text);
            let mut scalar = 0;
            let mut capitalize = true;
            let mut after_terminal = false;
            for grapheme in text.graphemes(true) {
                let count = grapheme.chars().count();
                if grapheme.chars().all(char::is_whitespace) {
                    if after_terminal {
                        capitalize = true;
                    }
                } else {
                    if capitalize {
                        for mapped in &mut mappings[scalar..scalar + count] {
                            *mapped = mapped.to_uppercase();
                        }
                    }
                    capitalize = false;
                }
                after_terminal = matches!(grapheme, "." | "?" | "!");
                scalar += count;
            }
            mappings
        }
    }
}
