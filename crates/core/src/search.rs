//! In-memory search and filtering over scanned skills.

use serde::{Deserialize, Serialize};

use crate::model::Skill;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Query {
    pub text: Option<String>,
    /// All listed tags must be present.
    pub tags: Vec<String>,
    pub category: Option<String>,
}

pub fn search<'a>(skills: &'a [Skill], q: &Query) -> Vec<&'a Skill> {
    let tokens: Vec<String> = q
        .text
        .as_deref()
        .unwrap_or("")
        .split_whitespace()
        .map(|t| t.to_lowercase())
        .collect();
    let wanted_tags: Vec<String> = q.tags.iter().map(|t| t.trim().to_lowercase()).collect();
    let wanted_cat = q
        .category
        .as_deref()
        .map(|c| c.trim().to_lowercase())
        .filter(|c| !c.is_empty());

    let mut scored: Vec<(i32, &Skill)> = skills
        .iter()
        .filter(|s| wanted_tags.iter().all(|t| s.tags.iter().any(|st| st == t)))
        .filter(|s| match &wanted_cat {
            Some(c) => s.category.as_deref().map(|sc| sc.to_lowercase()) == Some(c.clone()),
            None => true,
        })
        .filter_map(|s| {
            if tokens.is_empty() {
                return Some((0, s));
            }
            let name = s.name.to_lowercase();
            let id = s.id.to_lowercase();
            let desc = s.description.to_lowercase();
            let mut score = 0;
            for t in &tokens {
                let mut hit = 0;
                if id == *t || name == *t {
                    hit += 100;
                } else if id.contains(t) || name.contains(t) {
                    hit += 50;
                }
                if s.tags.iter().any(|tag| tag == t) {
                    hit += 40;
                } else if s.tags.iter().any(|tag| tag.contains(t)) {
                    hit += 20;
                }
                if s.category
                    .as_deref()
                    .map(|c| c.to_lowercase().contains(t))
                    .unwrap_or(false)
                {
                    hit += 15;
                }
                if desc.contains(t) {
                    hit += 10;
                }
                if hit == 0 {
                    return None;
                }
                score += hit;
            }
            Some((score, s))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    scored.into_iter().map(|(_, s)| s).collect()
}
