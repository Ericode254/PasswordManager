/// Case-insensitive subsequence matching, shared by the tree and quick picker.
pub fn matches(text: &str, query: &str) -> bool {
    let text = text.to_lowercase();
    let mut remaining = text.chars();
    query
        .to_lowercase()
        .chars()
        .all(|wanted| remaining.any(|c| c == wanted))
}

/// Prefer exact names and contiguous matches before broader subsequences.
pub fn rank(text: &str, query: &str) -> (u8, usize) {
    let text = text.to_lowercase();
    let query = query.to_lowercase();
    let name = text.rsplit('/').next().unwrap_or(&text);
    let priority = if name == query {
        0
    } else if name.starts_with(&query) {
        1
    } else if text.contains(&query) {
        2
    } else {
        3
    };
    (priority, text.len())
}

#[cfg(test)]
mod tests {
    #[test]
    fn matches_paths_and_unicode_in_order() {
        assert!(super::matches("Work/GitHub", "wgh"));
        assert!(super::matches("Jacket", "jk"));
        assert!(super::matches("École", "écl"));
        assert!(super::matches("anything", ""));
        assert!(!super::matches("GitHub", "hg"));
    }
}
