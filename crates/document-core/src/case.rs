//! Unicode casing with one output per original scalar, retaining run ownership.
use unicode_segmentation::UnicodeSegmentation;

/// Locale-independent Unicode case mappings. Title capitalizes the first
/// original cased grapheme of each whitespace-delimited token. Sentence
/// capitalizes the first cased grapheme and those after `.`, `?`, or `!`
/// followed by optional closing quotes/brackets and whitespace.
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

fn is_cased(grapheme: &str) -> bool {
    grapheme
        .chars()
        .any(|ch| ch.to_lowercase().ne(ch.to_uppercase()))
}

pub(crate) fn case_mappings(text: &str, case: TextCase) -> Vec<String> {
    match case {
        TextCase::Upper => text.chars().map(|ch| ch.to_uppercase().collect()).collect(),
        TextCase::Lower => lower_mappings(text),
        TextCase::Title => {
            let mut mappings = Vec::new();
            for token in text.split_inclusive(char::is_whitespace) {
                // Keep the complete token's context when lowercasing Greek sigma.
                let mut token_mappings = lower_mappings(token);
                let mut scalar = 0;
                for grapheme in token.graphemes(true) {
                    let count = grapheme.chars().count();
                    if is_cased(grapheme) {
                        for (mapped, ch) in token_mappings[scalar..scalar + count]
                            .iter_mut()
                            .zip(grapheme.chars())
                        {
                            *mapped = ch.to_uppercase().collect();
                        }
                        break;
                    }
                    scalar += count;
                }
                mappings.extend(token_mappings);
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
                } else if is_cased(grapheme) {
                    if capitalize {
                        for mapped in &mut mappings[scalar..scalar + count] {
                            *mapped = mapped.to_uppercase();
                        }
                    }
                    capitalize = false;
                    after_terminal = false;
                } else if matches!(grapheme, "." | "?" | "!") {
                    after_terminal = true;
                } else if !matches!(
                    grapheme,
                    "\"" | "'" | "”" | "’" | "»" | "›" | ")" | "]" | "}"
                ) {
                    after_terminal = false;
                }
                scalar += count;
            }
            mappings
        }
    }
}
