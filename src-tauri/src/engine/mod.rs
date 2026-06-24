use crate::ai::LlmResponse;

fn spellcheck_corrections(word: &str, count: usize) -> Option<Vec<String>> {
    if !crate::spellcheck::is_initialized() {
        return None;
    }

    let is_correct = crate::spellcheck::check(word).ok()?;

    if word.len() <= 3 || !is_correct {
        // Short input or misspelled — try prefix matching first
        let prefixes = crate::spellcheck::suggest_prefix_n(word, count).ok().unwrap_or_default();
        if prefixes.len() >= count {
            return Some(prefixes.into_iter().take(count).collect());
        }

        // Supplement with edit-distance
        let mut edits = crate::spellcheck::suggest_edit_n(word, count).ok().unwrap_or_default();
        edits.retain(|w| !prefixes.contains(w));
        let mut combined: Vec<String> = prefixes;
        combined.extend(edits.into_iter().take(count - combined.len()));
        if !combined.is_empty() {
            return Some(combined);
        }
        return None;
    }

    // Correct word — suggest alternatives
    let mut alts = crate::spellcheck::suggest_prefix_n(word, count).ok().unwrap_or_default();
    alts.retain(|w| w != word && !w.is_empty());
    let mut result = vec![word.to_string()];
    result.extend(alts.into_iter().take(count - 1));
    Some(result)
}

pub async fn get_suggestions(
    word: &str,
    count: usize,
) -> Result<LlmResponse, String> {
    let corrections = spellcheck_corrections(word, count);

    Ok(LlmResponse {
        corrections: corrections.unwrap_or_else(|| vec![word.to_string()]),
        emoji: String::new(),
        translation: None,
    })
}
