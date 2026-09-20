use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

#[derive(Default)]
pub struct FuzzyEngine {
    matcher: SkimMatcherV2,
}

impl FuzzyEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn find_best_matches<'a>(
        &self,
        query: &str,
        candidates: &'a [String],
        max_results: usize,
    ) -> Vec<(&'a String, i64)> {
        let mut scored: Vec<(&'a String, i64)> = candidates
            .iter()
            .filter_map(|cand| {
                self.matcher
                    .fuzzy_match(cand, query)
                    .map(|score| (cand, score))
            })
            .collect();

        // Trier par score décroissant
        scored.sort_by_key(|a| std::cmp::Reverse(a.1));
        scored.truncate(max_results);
        scored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_matching() {
        let engine = FuzzyEngine::new();
        let candidates = vec![
            "UserCreatedEvent".to_string(),
            "UserDeletedEvent".to_string(),
            "PostCreatedEvent".to_string(),
            "AuthService".to_string(),
        ];

        let results = engine.find_best_matches("UsrCreatd", &candidates, 2);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "UserCreatedEvent");
    }
}
