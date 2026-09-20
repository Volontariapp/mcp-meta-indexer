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
        let tokens: Vec<&str> = query.split_whitespace().collect();
        let mut scored: Vec<(&'a String, i64)> = candidates
            .iter()
            .filter_map(|cand| {
                if let Some(score) = self.matcher.fuzzy_match(cand, query) {
                    return Some((cand, score));
                }

                let cand_normalized = cand.replace(['-', '_'], " ");
                if let Some(score) = self.matcher.fuzzy_match(&cand_normalized, query) {
                    return Some((cand, score));
                }

                if tokens.len() > 1 {
                    let mut total_score = 0;
                    for token in &tokens {
                        let score = self.matcher.fuzzy_match(&cand_normalized, token)?;
                        total_score += score;
                    }
                    return Some((cand, total_score));
                }

                None
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

    #[test]
    fn test_fuzzy_matching_multi_token_hyphen() {
        let engine = FuzzyEngine::new();
        let candidates = vec![
            "3. Le Pattern Scatter-Gather (Ex: Création d'un Événement)".to_string(),
            "1. Le Transactional Outbox Pattern".to_string(),
            "2. Le Cycle de vie d'un Job".to_string(),
        ];

        let results = engine.find_best_matches("scattr gathr", &candidates, 2);
        assert!(!results.is_empty());
        assert_eq!(
            results[0].0,
            "3. Le Pattern Scatter-Gather (Ex: Création d'un Événement)"
        );
    }
}
