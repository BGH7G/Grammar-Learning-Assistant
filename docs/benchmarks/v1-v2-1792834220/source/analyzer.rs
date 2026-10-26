use crate::{
    analysis::{self, Analysis, Node, Piece},
    llm::{self, AttemptMetrics},
    settings::Settings,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Instant;

const PROMPT: &str = include_str!("prompts/analyze.txt");

#[derive(Deserialize)]
struct ModelAnalysis {
    nodes: Vec<Node>,
    translation: String,
    notes: Vec<String>,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisMetrics {
    pub total_ms: f64,
    pub configuration_ms: f64,
    pub preparation_ms: f64,
    pub attempts: Vec<AttemptMetrics>,
}

pub struct Analyzed {
    pub analysis: Analysis,
    pub metrics: AnalysisMetrics,
}

fn parse(raw: &str, pieces: &[Piece], text: &str) -> Result<Analysis, String> {
    // Only unwrap an exact code fence; never repair or complete JSON locally.
    let trimmed = raw.trim();
    let json = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|body| body.strip_suffix("```"))
        .unwrap_or(trimmed)
        .trim();
    let model: ModelAnalysis = serde_json::from_str(json).map_err(|e| {
        format!(
            "JSON 格式或字段类型错误（第 {} 行，第 {} 列）；必须包含 nodes、translation、notes。",
            e.line(),
            e.column()
        )
    })?;
    let result = Analysis {
        schema_version: "1".into(),
        pieces: pieces.to_vec(),
        nodes: model.nodes,
        translation: model.translation,
        notes: model.notes,
    };
    analysis::validate(&result, text)?;
    Ok(result)
}

pub async fn analyze(
    client: &reqwest::Client,
    s: &Settings,
    key: &str,
    text: &str,
) -> Result<Analyzed, String> {
    if text.trim().is_empty() || text.chars().count() > 2000 {
        return Err("请输入 1–2,000 个字符的英文句子。".into());
    }
    let started = Instant::now();
    let pieces = analysis::pieces(text);
    let input = json!({"text":text,"pieces":pieces.iter().map(|p|json!([p.id,p.text])).collect::<Vec<_>>()});
    let mut messages =
        json!([{"role":"system","content":PROMPT},{"role":"user","content":input.to_string()}]);
    let mut metrics = AnalysisMetrics {
        preparation_ms: started.elapsed().as_secs_f64() * 1000.0,
        ..AnalysisMetrics::default()
    };
    let budget = (2048 + text.chars().count() as u32 * 4).min(12000);
    for attempt in 0..2 {
        let mut response = llm::completion(client, s, key, messages.clone(), budget).await?;
        let validation_started = Instant::now();
        let parsed = parse(&response.content, &pieces, text);
        response.metrics.validation_ms = validation_started.elapsed().as_secs_f64() * 1000.0;
        match parsed {
            Ok(analysis) => {
                metrics.attempts.push(response.metrics);
                metrics.total_ms = started.elapsed().as_secs_f64() * 1000.0;
                return Ok(Analyzed { analysis, metrics });
            }
            Err(error) => {
                if attempt == 1 {
                    return Err(format!("模型修复后仍未通过校验：{error}"));
                }
                response.metrics.repair_reason = Some(error.clone());
                metrics.attempts.push(response.metrics);
                let list = messages.as_array_mut().unwrap();
                list.push(json!({"role":"assistant","content":response.content}));
                list.push(json!({"role":"user","content":format!("Fix only the following error and return the complete compact JSON, with the same input piece IDs: {error}")}));
            }
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    const VALID: &str = r#"{"nodes":[{"id":"s","parent_id":null,"kind":"clause","role":"root","first":0,"last":5},{"id":"a","parent_id":"s","kind":"phrase","role":"subject","first":0,"last":2},{"id":"b","parent_id":"s","kind":"word","role":"predicate","first":4,"last":4}],"translation":"这只猫在睡觉。","notes":[]}"#;

    #[test]
    fn output_never_overwrites_original_pieces() {
        let text = "The cat sleeps.";
        let a = parse(VALID, &analysis::pieces(text), text).unwrap();
        assert_eq!(
            a.pieces.iter().map(|p| p.text.as_str()).collect::<String>(),
            text
        );
        assert_eq!(a.nodes[1].role, "subject");
        let fenced = format!("```json\n{VALID}\n```");
        assert!(parse(&fenced, &analysis::pieces(text), text).is_ok());
        assert!(
            parse(
                &VALID.replace("\"last\":5", "\"last\":99"),
                &analysis::pieces(text),
                text
            )
            .err()
            .unwrap()
            .contains("范围")
        );
    }

    #[test]
    fn prompt_example_is_valid_and_matches_local_tokenizer() {
        let example = PROMPT
            .lines()
            .find_map(|l| l.strip_prefix("Example output: "))
            .unwrap();
        assert!(
            parse(
                example,
                &analysis::pieces("The cat sleeps."),
                "The cat sleeps."
            )
            .is_ok()
        );
    }

    fn run_mock(contents: &[&str]) -> Result<Analyzed, String> {
        use std::{
            io::{Read, Write},
            time::Duration,
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!(
            "http://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let responses: Vec<_>=contents.iter().map(|c|json!({"choices":[{"message":{"content":c},"finish_reason":"stop"}],"usage":{"completion_tokens":123}}).to_string()).collect();
        let server = std::thread::spawn(move || {
            for (attempt, body) in responses.iter().enumerate() {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((s, _)) => break s,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "Expected request {attempt}");
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(e) => panic!("{e}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut buf = [0; 4096];
                    let n = stream.read(&mut buf).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buf[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                        let len: usize = headers
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:"))
                            .unwrap()
                            .trim()
                            .parse()
                            .unwrap();
                        if bytes.len() >= end + 4 + len {
                            let sent: serde_json::Value =
                                serde_json::from_slice(&bytes[end + 4..end + 4 + len]).unwrap();
                            assert_eq!(sent["messages"].as_array().unwrap().len(), 2 + attempt * 2);
                            if attempt == 1 {
                                assert!(
                                    sent["messages"][3]["content"]
                                        .as_str()
                                        .unwrap()
                                        .contains("范围")
                                );
                            }
                            break;
                        }
                    }
                }
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
        });
        let s = Settings {
            endpoint,
            model: "mock".into(),
            ..Settings::default()
        };
        let result = tauri::async_runtime::block_on(analyze(
            &reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            &s,
            "",
            "The cat sleeps.",
        ));
        server.join().unwrap();
        result
    }

    #[test]
    fn retries_once_with_specific_validation_error_and_exposes_timings() {
        let bad = VALID.replace("\"last\":5", "\"last\":99");
        let a = run_mock(&[&bad, VALID]).ok().unwrap();
        assert_eq!(a.metrics.attempts.len(), 2);
        assert!(
            a.metrics.attempts[0]
                .repair_reason
                .as_ref()
                .unwrap()
                .contains("范围")
        );
        assert_eq!(a.metrics.attempts[1].output_tokens, Some(123));
        assert!(a.metrics.total_ms >= a.metrics.attempts.iter().map(|m| m.request_ms).sum::<f64>());
        assert!(run_mock(&[&bad, &bad]).is_err());
    }

    #[test]
    fn valid_output_does_not_trigger_a_second_request() {
        assert_eq!(run_mock(&[VALID]).ok().unwrap().metrics.attempts.len(), 1);
    }
}
