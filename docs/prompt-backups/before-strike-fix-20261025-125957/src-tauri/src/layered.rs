use crate::{
    analysis::{self, Piece},
    analyzer::AnalysisMetrics,
    llm,
    settings::Settings,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Instant,
};

pub const VERSION: &str = "layer-v2.7-user";
pub const MAX_DEPTH: u8 = 4;
const PROMPT: &str = include_str!("prompts/layer-v2.txt");
const MAX_SESSIONS: usize = 8;
const MAX_LAYERS: usize = 48;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub id: usize,
    pub text: String,
    pub piece_id: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub role: String,
    pub ctype: String,
    pub first: usize,
    pub last: usize,
    pub expandable: bool,
    pub depth: u8,
    pub sentence_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    pub source_id: String,
    pub head_token: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub parent_id: String,
    pub depth: u8,
    pub skeleton: Option<String>,
    pub translation: String,
    pub nodes: Vec<Node>,
    pub relations: Vec<Relation>,
    pub notes: Vec<String>,
    pub warnings: Vec<String>,
    pub normalization_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub schema_version: String,
    pub prompt_version: String,
    pub session_id: String,
    pub pieces: Vec<Piece>,
    pub tokens: Vec<Token>,
    pub roots: Vec<Node>,
    pub layers: Vec<Layer>,
    pub max_depth: u8,
}

#[derive(Clone)]
pub struct Session {
    pub id: String,
    pub config_key: String,
    pub pieces: Vec<Piece>,
    pub tokens: Vec<Token>,
    pub roots: Vec<Node>,
    pub nodes: HashMap<String, Node>,
    pub layers: HashMap<String, Layer>,
    in_flight: HashSet<String>,
}

#[derive(Default)]
pub struct Store {
    sessions: HashMap<String, Session>,
    order: VecDeque<String>,
}

pub enum Plan {
    Cached(Layer),
    Fetch(Box<Session>, Node),
}

impl Store {
    pub fn clear(&mut self) {
        self.sessions.clear();
        self.order.clear();
    }
    pub fn insert(&mut self, session: Session) {
        while self.order.len() >= MAX_SESSIONS {
            if let Some(id) = self.order.pop_front() {
                self.sessions.remove(&id);
            }
        }
        self.order.push_back(session.id.clone());
        self.sessions.insert(session.id.clone(), session);
    }
    pub fn plan(
        &mut self,
        session_id: &str,
        node_id: &str,
        config_key: &str,
    ) -> Result<Plan, String> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or("会话已过期，请重新分析。")?;
        if session.config_key != config_key {
            return Err("模型配置已变化，请重新分析。".into());
        }
        if let Some(layer) = session.layers.get(node_id) {
            return Ok(Plan::Cached(layer.clone()));
        }
        let node = session
            .nodes
            .get(node_id)
            .ok_or("节点不属于当前会话。")?
            .clone();
        if node.depth >= MAX_DEPTH {
            return Err("已达到四层解析上限。".into());
        }
        if node.first == node.last {
            return Err("单个词或标点不需要继续展开。".into());
        }
        if session.layers.len() + session.in_flight.len() >= MAX_LAYERS {
            return Err("本次会话已达到解析预算，请重新分析或缩短输入。".into());
        }
        if !session.in_flight.insert(node_id.to_owned()) {
            return Err("此节点正在解析，请稍候。".into());
        }
        Ok(Plan::Fetch(Box::new(session.clone()), node))
    }
    pub fn finish(
        &mut self,
        session_id: &str,
        node_id: &str,
        config_key: &str,
        layer: Option<Layer>,
    ) -> Result<(), String> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or("会话已失效，请重新分析。")?;
        session.in_flight.remove(node_id);
        if session.config_key != config_key {
            return Err("模型配置已变化，请重新分析。".into());
        }
        if let Some(layer) = layer {
            session.attach(layer);
        }
        Ok(())
    }
}

impl Session {
    pub fn prepare(
        id: String,
        text: &str,
        config_key: String,
        split: bool,
    ) -> Result<Self, String> {
        if text.trim().is_empty() || text.chars().count() > 2000 {
            return Err("请输入 1–2,000 个字符的英文。".into());
        }
        let pieces = analysis::pieces(text);
        let tokens: Vec<_> = pieces
            .iter()
            .filter(|p| !p.text.chars().all(char::is_whitespace))
            .enumerate()
            .map(|(id, p)| Token {
                id,
                text: p.text.clone(),
                piece_id: p.id,
            })
            .collect();
        if tokens.is_empty() {
            return Err("请输入有效英文。".into());
        }
        let ranges = if split {
            sentence_ranges(&tokens, &pieces)
        } else {
            vec![(0, tokens.len() - 1)]
        };
        if ranges.len() > 8 {
            return Err("一次最多分析 8 句，请分批输入或关闭自动分句。".into());
        }
        let roots: Vec<_> = ranges
            .into_iter()
            .enumerate()
            .map(|(i, (first, last))| {
                let node_id = format!("{id}:s{i}");
                Node {
                    id: node_id.clone(),
                    parent_id: None,
                    kind: "sentence".into(),
                    role: "root".into(),
                    ctype: "none".into(),
                    first,
                    last,
                    expandable: true,
                    depth: 0,
                    sentence_id: node_id,
                }
            })
            .collect();
        Ok(Self {
            id,
            config_key,
            pieces,
            tokens,
            nodes: roots.iter().map(|n| (n.id.clone(), n.clone())).collect(),
            roots,
            layers: HashMap::new(),
            in_flight: HashSet::new(),
        })
    }
    pub fn attach(&mut self, layer: Layer) {
        for n in &layer.nodes {
            self.nodes.insert(n.id.clone(), n.clone());
        }
        self.layers.insert(layer.parent_id.clone(), layer);
    }
    pub fn document(&self) -> Document {
        Document {
            schema_version: "2".into(),
            prompt_version: VERSION.into(),
            session_id: self.id.clone(),
            pieces: self.pieces.clone(),
            tokens: self.tokens.clone(),
            roots: self.roots.clone(),
            layers: self
                .roots
                .iter()
                .filter_map(|r| self.layers.get(&r.id).cloned())
                .collect(),
            max_depth: MAX_DEPTH,
        }
    }
    fn sentence_text(&self, node: &Node) -> String {
        let root = self.nodes.get(&node.sentence_id).unwrap();
        self.pieces[self.tokens[root.first].piece_id..=self.tokens[root.last].piece_id]
            .iter()
            .map(|p| p.text.as_str())
            .collect()
    }
}

// Conservative splitting; abbreviations/decimals and closing quotes stay together.
// The UI exposes a manual single-sentence mode for ambiguous boundaries.
fn sentence_ranges(tokens: &[Token], pieces: &[Piece]) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < tokens.len() {
        let t = &tokens[i].text;
        if ![".", "?", "!"].contains(&t.as_str()) {
            i += 1;
            continue;
        }
        if t == "." && i > 0 && i + 1 < tokens.len() {
            let prev = &tokens[i - 1];
            let next = &tokens[i + 1];
            let abbreviation = [
                "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "vs", "etc", "e", "g", "i.e",
            ]
            .contains(&prev.text.to_lowercase().as_str());
            let decimal = prev.text.chars().all(|c| c.is_ascii_digit())
                && next.text.chars().all(|c| c.is_ascii_digit())
                && prev.piece_id + 1 == tokens[i].piece_id
                && tokens[i].piece_id + 1 == next.piece_id;
            let initial = prev.text.chars().count() == 1
                && prev.text.chars().all(char::is_uppercase)
                && prev.text != "I";
            if abbreviation || decimal || initial {
                i += 1;
                continue;
            }
        }
        let mut end = i;
        while end + 1 < tokens.len()
            && [".", "!", "?", "\"", "”", "'", "’", ")", "]"]
                .contains(&tokens[end + 1].text.as_str())
        {
            end += 1;
        }
        let separated = end + 1 == tokens.len()
            || tokens[end].piece_id + 1 < tokens[end + 1].piece_id
                && pieces[tokens[end].piece_id + 1..tokens[end + 1].piece_id]
                    .iter()
                    .any(|p| p.text.chars().any(char::is_whitespace));
        if separated || t != "." {
            result.push((start, end));
            start = end + 1;
        }
        i = end + 1;
    }
    if start < tokens.len() {
        result.push((start, tokens.len() - 1));
    }
    result
}

#[derive(Deserialize)]
struct RawLayer {
    v: u8,
    #[serde(default)]
    sk: Option<String>,
    tr: String,
    n: Vec<(String, String, String, usize, usize, u8)>,
    #[serde(default)]
    r: Vec<(usize, usize)>,
    notes: Vec<String>,
}

pub fn decode(raw: &str, parent: &Node) -> Result<Layer, String> {
    let trimmed = raw.trim();
    let body = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .unwrap_or(trimmed)
        .trim();
    let mut data: RawLayer = serde_json::from_str(body).map_err(|e| {
        format!(
            "v2 JSON 字段或六项节点数组错误（第 {} 行，第 {} 列）。",
            e.line(),
            e.column()
        )
    })?;
    if data.v != 2 {
        return Err("v 必须为数字 2。".into());
    }
    if data.tr.trim().is_empty() || data.tr.chars().count() > 3000 {
        return Err("tr 必须是当前片段的完整简短译文。".into());
    }
    let mut warnings = Vec::new();
    let note_count = data.notes.len();
    data.notes.retain(|n| n.chars().count() <= 60);
    data.notes.truncate(2);
    if data.notes.len() != note_count {
        warnings.push("部分过长或多余的说明已省略。".into());
    }
    let depth = parent.depth + 1;
    if depth > MAX_DEPTH {
        return Err("超过四层解析上限。".into());
    }
    if depth == 1 && data.sk.as_ref().is_none_or(|s| s.trim().is_empty()) {
        return Err("首层须提供主干概述 sk。".into());
    }
    if depth == 1
        && data
            .sk
            .as_ref()
            .is_some_and(|s| s.split_whitespace().count() > 35)
    {
        data.sk = None;
        warnings.push("主干概述过长，已省略；请查看原文结构。".into());
    }
    if data.n.len() > parent.last - parent.first + 1 {
        return Err("节点数超过目标 token 数量。".into());
    }
    if data.n.len() == 1 && data.n[0].3 == parent.first && data.n[0].4 == parent.last {
        return Err(
            "没有缩小目标范围；请返回更小的直接成分，或 n=[] 并说明已无法继续拆分。".into(),
        );
    }
    if data.n.is_empty() && data.notes.is_empty() {
        if warnings.is_empty() {
            return Err("n=[] 时须用 notes 简短说明无法继续拆分的原因。".into());
        }
        data.notes.push("该片段未返回更细的有效拆分。".into());
    }
    let mut next = parent.first;
    let mut nodes = Vec::new();
    let mut normalization_count = 0;
    for (i, (mut kind, mut role, ctype, first, last, expandable)) in data.n.into_iter().enumerate()
    {
        // Some compatible models return a part-of-speech name in the kind slot.
        // For a single token this is a representation alias for `word`; never
        // change ranges, roles or clause/phrase structure to force acceptance.
        if first == last
            && [
                "conjunction",
                "punctuation",
                "preposition",
                "noun",
                "verb",
                "adjective",
                "adverb",
                "determiner",
                "pronoun",
            ]
            .contains(&kind.as_str())
        {
            kind = "word".into();
            normalization_count += 1;
        }
        if role == "main" && kind == "clause" && ctype == "main" {
            role = "main_clause".into();
            normalization_count += 1;
        }
        if !["clause", "phrase", "word"].contains(&kind.as_str())
            || ![
                "main_clause",
                "subject",
                "predicate",
                "object",
                "attribute",
                "adverbial",
                "complement",
                "predicative",
                "conjunction",
                "coordinate",
                "head",
                "determiner",
                "marker",
                "punctuation",
                "other",
            ]
            .contains(&role.as_str())
            || ![
                "main",
                "coordinate",
                "relative",
                "noun",
                "adverbial",
                "infinitive",
                "participial",
                "gerund",
                "none",
            ]
            .contains(&ctype.as_str())
        {
            return Err(format!(
                "n[{i}] kind={kind:?}, role={role:?}, ctype={ctype:?} 不在枚举中；请逐项核对枚举。"
            ));
        }
        if first != next || first > last || last > parent.last || expandable > 1 {
            return Err(format!(
                "n[{i}] actual first={first}, last={last}, expandable={expandable}; REQUIRED first={next}, {next}<=last<={}, expandable=0/1. Correct this array; do not repeat it.",
                parent.last
            ));
        }
        next = last + 1;
        nodes.push(Node {
            id: format!("{}:{i}", parent.id),
            parent_id: Some(parent.id.clone()),
            kind,
            role,
            ctype,
            first,
            last,
            expandable: expandable == 1 && first < last && depth < MAX_DEPTH,
            depth,
            sentence_id: parent.sentence_id.clone(),
        });
    }
    if !nodes.is_empty() && next != parent.last + 1 {
        return Err(format!(
            "节点未完全覆盖目标，最后一个 last 应为 {}。",
            parent.last
        ));
    }
    let mut relations = Vec::new();
    let mut seen = HashSet::new();
    for (source, head) in data.r {
        let valid = nodes.get(source).is_some_and(|node| {
            head >= parent.first && head <= parent.last && !(node.first..=node.last).contains(&head)
        });
        if !valid || !seen.insert((source, head)) {
            if warnings
                .iter()
                .all(|s| s != "部分修饰关联未通过校验，已省略连线。")
            {
                warnings.push("部分修饰关联未通过校验，已省略连线。".into());
            }
            continue;
        }
        relations.push(Relation {
            source_id: nodes[source].id.clone(),
            head_token: head,
        });
    }
    Ok(Layer {
        parent_id: parent.id.clone(),
        depth,
        skeleton: if depth == 1 { data.sk } else { None },
        translation: data.tr,
        nodes,
        relations,
        notes: data.notes,
        warnings,
        normalization_count,
    })
}

#[derive(Serialize)]
pub struct QueryFailure {
    pub message: String,
    pub metrics: AnalysisMetrics,
}
impl From<QueryFailure> for String {
    fn from(f: QueryFailure) -> String {
        f.message
    }
}

// Preserve absolute IDs and include the entire containing sentence for disambiguation.
fn query_input(session: &Session, parent: &Node) -> serde_json::Value {
    let root = session.nodes.get(&parent.sentence_id).unwrap();
    json!({
        "tokens": session.tokens[root.first..=root.last].iter()
            .map(|token| format!("{}:{}", token.id, token.text)).collect::<Vec<_>>(),
        "target": [parent.first, parent.last],
        "depth": parent.depth + 1,
        "parent": {"kind": parent.kind, "role": parent.role, "ctype": parent.ctype}
    })
}

pub async fn query(
    client: &reqwest::Client,
    s: &Settings,
    key: &str,
    session: &Session,
    parent: &Node,
) -> Result<(Layer, AnalysisMetrics), QueryFailure> {
    let started = Instant::now();
    let input = query_input(session, parent);
    let mut messages =
        json!([{"role":"system","content":PROMPT},{"role":"user","content":input.to_string()}]);
    let mut metrics = AnalysisMetrics {
        preparation_ms: started.elapsed().as_secs_f64() * 1000.0,
        ..AnalysisMetrics::default()
    };
    let budget = (1024 + (parent.last - parent.first + 1) as u32 * 24).min(6000);
    for attempt in 0..2 {
        let request_started = Instant::now();
        let mut response = match llm::completion(client, s, key, messages.clone(), budget).await {
            Ok(response) => response,
            Err(message) => {
                metrics.attempts.push(llm::AttemptMetrics {
                    request_ms: request_started.elapsed().as_secs_f64() * 1000.0,
                    stage: Some(format!("第{}层 · 请求失败", parent.depth + 1)),
                    ..llm::AttemptMetrics::default()
                });
                metrics.total_ms = started.elapsed().as_secs_f64() * 1000.0;
                return Err(QueryFailure { message, metrics });
            }
        };
        response.metrics.stage = Some(format!(
            "第{}层 · {}–{} · {}",
            parent.depth + 1,
            parent.first,
            parent.last,
            if attempt == 0 { "初次" } else { "修复" }
        ));
        let validate_started = Instant::now();
        let parsed = decode(&response.content, parent);
        response.metrics.validation_ms = validate_started.elapsed().as_secs_f64() * 1000.0;
        match parsed {
            Ok(layer) => {
                metrics.attempts.push(response.metrics);
                metrics.total_ms = started.elapsed().as_secs_f64() * 1000.0;
                return Ok((layer, metrics));
            }
            Err(error) => {
                response.metrics.repair_reason = Some(error.clone());
                metrics.attempts.push(response.metrics);
                if attempt == 1 {
                    metrics.total_ms = started.elapsed().as_secs_f64() * 1000.0;
                    return Err(QueryFailure {
                        message: format!("本层修复后仍未通过校验：{error} 上一层结果已保留。"),
                        metrics,
                    });
                }
                messages.as_array_mut().unwrap().extend([json!({"role":"assistant","content":response.content}),json!({"role":"user","content":format!("Fix this validation error and return the complete v2 JSON for the same target only: {error}")})]);
            }
        }
    }
    unreachable!()
}

pub async fn first_layers(
    client: &reqwest::Client,
    s: &Settings,
    key: &str,
    session: &mut Session,
) -> Result<AnalysisMetrics, String> {
    let started = Instant::now();
    let mut metrics = AnalysisMetrics::default();
    // At most two sentence requests concurrently; no automatic expansion/prefetch.
    for roots in session.roots.clone().chunks(2) {
        let mut jobs = Vec::new();
        for root in roots {
            let (client, s, key, copy, root) = (
                client.clone(),
                s.clone(),
                key.to_owned(),
                session.clone(),
                root.clone(),
            );
            jobs.push(tauri::async_runtime::spawn(async move {
                query(&client, &s, &key, &copy, &root).await
            }));
        }
        let mut completed = Vec::new();
        for job in jobs {
            completed.push(job.await.map_err(|_| "句子解析任务异常。".to_owned())?);
        }
        for result in completed {
            let (layer, mut m) = result?;
            metrics.preparation_ms += m.preparation_ms;
            metrics.attempts.append(&mut m.attempts);
            session.attach(layer);
        }
    }
    metrics.total_ms = started.elapsed().as_secs_f64() * 1000.0;
    Ok(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;
    const VALID: &str = r#"{"v":2,"sk":"The cat sleeps.","tr":"猫在睡觉。","n":[["phrase","subject","none",0,1,0],["word","predicate","none",2,2,0],["word","punctuation","none",3,3,0]],"r":[],"notes":[]}"#;
    fn session(id: &str, text: &str) -> Session {
        Session::prepare(id.into(), text, "config-a".into(), true).unwrap()
    }
    #[test]
    fn source_mapping_is_lossless_and_ids_are_scoped_by_session_and_parent() {
        let a = session("a", "The cat sleeps.");
        let b = session("b", "The cat sleeps.");
        let la = decode(VALID, &a.roots[0]).unwrap();
        let lb = decode(VALID, &b.roots[0]).unwrap();
        assert_ne!(la.nodes[0].id, lb.nodes[0].id);
        let original = "  Café\tcan't\r\nstop—really! 😀  ";
        let a = session("unicode", original);
        assert_eq!(
            a.pieces.iter().map(|p| p.text.as_str()).collect::<String>(),
            original
        );
        for token in &a.tokens {
            assert_eq!(token.text, a.pieces[token.piece_id].text);
        }
    }
    #[test]
    fn checks_coverage_overlap_enum_and_forward_progress() {
        let a = session("a", "The cat sleeps.");
        let root = &a.roots[0];
        for bad in [
            VALID.replace("0,1,0", "1,1,0"),
            VALID.replace("2,2,0", "1,2,0"),
            VALID.replace("3,3,0", "3,9,0"),
            VALID.replace("subject", "unknown"),
            VALID.replace("0,1,0", "0,1,2"),
        ] {
            assert!(decode(&bad, root).is_err());
        }
        let repeat = r#"{"v":2,"sk":"cat sleeps","tr":"猫睡觉。","n":[["clause","coordinate","main",0,3,1]],"notes":[]}"#;
        assert!(decode(repeat, root).err().unwrap().contains("缩小"));
        let terminal =
            r#"{"v":2,"sk":"cat sleeps","tr":"猫睡觉。","n":[],"notes":["无需进一步拆分。"]}"#;
        assert!(decode(terminal, root).is_ok());
    }
    #[test]
    fn validates_relation_edges_and_enforces_depth_cap() {
        let a = session("a", "The report published yesterday helped.");
        let raw = r#"{"v":2,"sk":"The report helped.","tr":"报告有用。","n":[["phrase","subject","none",0,1,0],["phrase","attribute","participial",2,3,1],["word","predicate","none",4,4,0],["word","punctuation","none",5,5,0]],"r":[[1,1]],"notes":[]}"#;
        let layer = decode(raw, &a.roots[0]).unwrap();
        assert_eq!(layer.relations[0].head_token, 1);
        let omitted = decode(&raw.replace("[1,1]", "[1,2]"), &a.roots[0]).unwrap();
        assert!(omitted.relations.is_empty());
        assert!(!omitted.warnings.is_empty());
        let mut deep = a.roots[0].clone();
        deep.depth = 3;
        assert!(
            decode(raw, &deep)
                .unwrap()
                .nodes
                .iter()
                .all(|n| !n.expandable)
        );
        deep.depth = 4;
        assert!(decode(raw, &deep).is_err());
    }
    #[test]
    fn only_normalizes_single_token_pos_aliases() {
        let a = session("alias", "The cat sleeps.");
        let raw = VALID.replace(
            "[\"word\",\"punctuation\"",
            "[\"punctuation\",\"punctuation\"",
        );
        let layer = decode(&raw, &a.roots[0]).unwrap();
        assert_eq!(layer.nodes[2].kind, "word");
        assert_eq!(layer.normalization_count, 1);
        let invalid = VALID.replace("[\"phrase\",\"subject\"", "[\"noun\",\"subject\"");
        assert!(decode(&invalid, &a.roots[0]).is_err());
    }
    #[test]
    fn cache_rejects_stale_configuration_duplicates_and_expired_sessions() {
        let mut store = Store::default();
        let mut a = session("a", "The cat sleeps.");
        let layer = decode(VALID, &a.roots[0]).unwrap();
        let target = layer.nodes[0].id.clone();
        a.attach(layer);
        store.insert(a);
        assert!(matches!(
            store.plan("a", "a:s0", "config-a"),
            Ok(Plan::Cached(_))
        ));
        assert!(store.plan("a", "a:s0", "config-b").is_err());
        assert!(matches!(
            store.plan("a", &target, "config-a"),
            Ok(Plan::Fetch(_, _))
        ));
        assert!(store.plan("a", &target, "config-a").is_err());
        store.finish("a", &target, "config-a", None).unwrap();
        assert!(matches!(
            store.plan("a", &target, "config-a"),
            Ok(Plan::Fetch(_, _))
        ));
        store.clear();
        assert!(store.plan("a", &target, "config-a").is_err());
    }
    #[test]
    fn sentence_split_preserves_decimals_abbreviations_and_quotes() {
        let a = session("a", "Dr. Smith paid $3.50. He said, “Go!” Then he left.");
        assert_eq!(a.roots.len(), 3);
        let texts: Vec<_> = a.roots.iter().map(|n| a.sentence_text(n)).collect();
        assert_eq!(
            texts,
            vec!["Dr. Smith paid $3.50.", "He said, “Go!”", "Then he left."]
        );
        let a = Session::prepare("b".into(), "First. Second.", "c".into(), false).unwrap();
        assert_eq!(a.roots.len(), 1);
    }
    #[test]
    fn expansion_input_keeps_sentence_context_and_absolute_ids() {
        let a = Session::prepare(
            "input".into(),
            "First. The bird sings.",
            "config".into(),
            true,
        )
        .unwrap();
        let root = &a.roots[1];
        let mut child = root.clone();
        child.first = root.first + 1;
        child.last = root.last - 1;
        child.depth = 1;
        child.role = "predicate".into();
        let input = query_input(&a, &child);
        assert_eq!(input["target"], json!([child.first, child.last]));
        assert_eq!(input["depth"], 2);
        assert_eq!(input["parent"]["role"], "predicate");
        assert!(input.get("role").is_none());
        let tokens = input["tokens"].as_array().unwrap();
        assert_eq!(tokens.len(), root.last - root.first + 1);
        assert_eq!(tokens[0], format!("{}:The", root.first));
        assert_eq!(tokens.last().unwrap(), &json!(format!("{}:.", root.last)));
        assert!(!tokens.iter().any(|t| t.as_str().unwrap().contains("First")));
    }

    #[test]
    fn prompt_example_and_note_limits() {
        let a = session("a", "The cat sleeps.");
        // The supplied prompt uses attached punctuation in its illustrative token lists.
        // Validate its full JSON examples against those declared absolute ranges.
        for (marker, first, last, depth) in [
            ("Ex1 (", 0, 13, 0),
            ("depth 2 [2,13]:", 2, 13, 1),
            ("Ex2 (", 0, 9, 0),
        ] {
            let section = &PROMPT[PROMPT.find(marker).unwrap()..];
            let json_start = section.find("{\"v\":2").unwrap();
            let value = serde_json::Deserializer::from_str(&section[json_start..])
                .into_iter::<serde_json::Value>()
                .next()
                .unwrap()
                .unwrap();
            let mut parent = a.roots[0].clone();
            parent.first = first;
            parent.last = last;
            parent.depth = depth;
            assert!(decode(&value.to_string(), &parent).is_ok(), "{marker}");
        }
        let bad = VALID.replace(
            "\"notes\":[]",
            &format!("\"notes\":[\"{}\"]", "长".repeat(61)),
        );
        let limited = decode(&bad, &a.roots[0]).unwrap();
        assert!(limited.notes.is_empty());
        assert!(!limited.warnings.is_empty());
    }
}
