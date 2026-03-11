use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub command: String,
}

fn skills_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config/claude-cage/skills.json")
}

pub fn load_skills() -> Vec<Skill> {
    let path = skills_path();
    if let Ok(data) = fs::read_to_string(&path) {
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        Vec::new()
    }
}

pub fn save_skills(skills: &[Skill]) {
    let path = skills_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_string_pretty(skills) {
        let _ = fs::write(&path, data);
    }
}

/// Fuzzy match: all characters of the query must appear in order in the name.
pub fn fuzzy_match(query: &str, name: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query_lower = query.to_lowercase();
    let name_lower = name.to_lowercase();
    let mut chars = query_lower.chars();
    let mut current = chars.next();
    for c in name_lower.chars() {
        if let Some(q) = current {
            if c == q {
                current = chars.next();
            }
        } else {
            break;
        }
    }
    current.is_none()
}

/// Score a fuzzy match — lower is better. Returns None if no match.
pub fn fuzzy_score(query: &str, name: &str) -> Option<usize> {
    if query.is_empty() {
        return Some(0);
    }
    let query_lower = query.to_lowercase();
    let name_lower = name.to_lowercase();
    let mut score = 0;
    let mut last_idx: Option<usize> = None;
    let mut qi = query_lower.chars();
    let mut current = qi.next()?;

    for (i, c) in name_lower.chars().enumerate() {
        if c == current {
            // Bonus for consecutive matches
            if let Some(li) = last_idx {
                score += i - li - 1;
            } else {
                score += i; // penalize late start
            }
            last_idx = Some(i);
            match qi.next() {
                Some(next) => current = next,
                None => return Some(score),
            }
        }
    }
    None // not all query chars matched
}

pub fn filter_and_sort<'a>(skills: &'a [Skill], query: &str) -> Vec<(usize, &'a Skill)> {
    let mut matches: Vec<(usize, usize, &Skill)> = skills
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            fuzzy_score(query, &s.name).map(|score| (i, score, s))
        })
        .collect();
    matches.sort_by_key(|(_, score, _)| *score);
    matches.into_iter().map(|(i, _, s)| (i, s)).collect()
}
