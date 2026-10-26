use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
#[derive(Clone, Serialize, Deserialize)]
pub struct Piece {
    pub id: usize,
    pub text: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub role: String,
    pub first: usize,
    pub last: usize,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Analysis {
    pub schema_version: String,
    pub pieces: Vec<Piece>,
    pub nodes: Vec<Node>,
    pub translation: String,
    pub notes: Vec<String>,
}
/// Split locally without normalizing or losing any Unicode, whitespace or punctuation.
/// English words (including contractions), whitespace runs and punctuation get stable IDs.
pub fn pieces(text: &str) -> Vec<Piece> {
    let mut result: Vec<Piece> = Vec::new();
    let mut previous = None;
    for c in text.chars() {
        let class = if c.is_whitespace() {
            0
        } else if c.is_alphanumeric() || c == char::from(39) || c == '’' {
            1
        } else {
            2
        };
        if class != 2 && previous == Some(class) {
            result.last_mut().unwrap().text.push(c);
        } else {
            result.push(Piece {
                id: result.len(),
                text: c.to_string(),
            });
        }
        previous = Some(class);
    }
    result
}

pub fn validate(a: &Analysis, text: &str) -> Result<(), String> {
    if a.schema_version != "1" || a.pieces.is_empty() || a.nodes.is_empty() || a.nodes.len() > 4000
    {
        return Err("schema_version 须为 1，pieces/nodes 非空，nodes 不超过 4000 项。".into());
    }
    if a.pieces
        .iter()
        .enumerate()
        .any(|(i, p)| p.id != i || p.text.is_empty())
        || a.pieces.iter().map(|p| p.text.as_str()).collect::<String>() != text
    {
        return Err("pieces 的 ID 须连续，且拼接文字必须与原文完全相同。".into());
    }
    let mut ids = HashMap::new();
    for (i, n) in a.nodes.iter().enumerate() {
        if n.id.is_empty() || ids.insert(n.id.as_str(), i).is_some() {
            return Err(format!("nodes[{i}].id 为空或重复。"));
        }
    }
    let roots: Vec<_> = a.nodes.iter().filter(|n| n.parent_id.is_none()).collect();
    if roots.len() != 1 {
        return Err("必须恰有一个 parent_id=null 的根节点。".into());
    }
    let root = roots[0];
    if root.role != "root"
        || root.kind != "clause"
        || root.first != 0
        || root.last != a.pieces.len() - 1
    {
        return Err(format!(
            "根节点须为 clause/root，范围须为 0..{}。",
            a.pieces.len() - 1
        ));
    }
    let mut siblings: HashMap<Option<&str>, Vec<(usize, usize, usize)>> = HashMap::new();
    for (i, n) in a.nodes.iter().enumerate() {
        if n.first > n.last || n.last >= a.pieces.len() {
            return Err(format!(
                "nodes[{i}] 范围越界：须满足 0 <= first <= last <= {}。",
                a.pieces.len() - 1
            ));
        }
        if !["clause", "phrase", "word"].contains(&n.kind.as_str())
            || ![
                "root",
                "subject",
                "predicate",
                "object",
                "attribute",
                "adverbial",
                "complement",
                "predicative",
            ]
            .contains(&n.role.as_str())
            || (n.parent_id.is_some() && n.role == "root")
        {
            return Err(format!(
                "nodes[{i}] 的 kind/role 不在约定枚举中，或子节点使用了 root。"
            ));
        }
        let mut current = i;
        let mut seen = HashSet::new();
        while let Some(parent_id) = &a.nodes[current].parent_id {
            if !seen.insert(current) {
                return Err(format!("nodes[{i}] 的祖先链存在循环。"));
            }
            let parent = *ids
                .get(parent_id.as_str())
                .ok_or_else(|| format!("nodes[{current}].parent_id 指向不存在的节点。"))?;
            if a.nodes[current].first < a.nodes[parent].first
                || a.nodes[current].last > a.nodes[parent].last
            {
                return Err(format!("nodes[{current}] 范围超出父节点 nodes[{parent}]。"));
            }
            current = parent;
        }
        siblings
            .entry(n.parent_id.as_deref())
            .or_default()
            .push((n.first, n.last, i));
    }
    for group in siblings.values_mut() {
        group.sort_unstable();
        for pair in group.windows(2) {
            if pair[0].1 >= pair[1].0 {
                return Err(format!(
                    "同级 nodes[{}] 与 nodes[{}] 范围重叠；嵌套结构须建立父子关系。",
                    pair[0].2, pair[1].2
                ));
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_pieces_preserve_contractions_unicode_and_repeated_whitespace() {
        for text in [
            "The cat sleeps.",
            "I can't go—can you?",
            "“Café”  isn’t\n猫 😀!",
            "",
            "\t\r\n",
        ] {
            let p = pieces(text);
            assert_eq!(p.iter().map(|p| p.text.as_str()).collect::<String>(), text);
            assert!(
                p.iter()
                    .enumerate()
                    .all(|(i, p)| p.id == i && !p.text.is_empty())
            );
        }
        assert_eq!(
            pieces("The cat sleeps.")
                .iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>(),
            vec!["The", " ", "cat", " ", "sleeps", "."]
        );
    }
    fn sample() -> Analysis {
        serde_json::from_str(r#"{"schema_version":"1","pieces":[{"id":0,"text":"猫 😀\n"}],"nodes":[{"id":"s","parent_id":null,"kind":"clause","role":"root","first":0,"last":0}],"translation":"猫","notes":[]}"#).unwrap()
    }
    #[test]
    fn preserves_unicode_and_whitespace() {
        assert!(validate(&sample(), "猫 😀\n").is_ok());
        assert!(validate(&sample(), "猫 😀").is_err());
    }
    #[test]
    fn rejects_invalid_ranges_and_ids() {
        let mut a = sample();
        a.pieces[0].id = 1;
        assert!(validate(&a, "猫 😀\n").is_err());
        let mut a = sample();
        a.nodes[0].last = 2;
        assert!(validate(&a, "猫 😀\n").is_err());
    }
    #[test]
    fn rejects_cycles_and_overlap() {
        let mut a = sample();
        let mut n = a.nodes[0].clone();
        n.id = "a".into();
        n.role = "subject".into();
        n.parent_id = Some("a".into());
        a.nodes.push(n);
        assert!(validate(&a, "猫 😀\n").is_err());
        a.nodes[1].parent_id = Some("s".into());
        assert!(validate(&a, "猫 😀\n").is_ok());
        let mut n = a.nodes[1].clone();
        n.id = "b".into();
        a.nodes.push(n);
        assert!(validate(&a, "猫 😀\n").is_err());
    }
}
