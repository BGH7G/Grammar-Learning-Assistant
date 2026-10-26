use crate::settings::{ReasoningEffort, Settings, ThinkingProtocol};
use serde::Serialize;
use serde_json::{Value, json};
use std::time::Instant;

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptMetrics {
    pub request_ms: f64,
    pub response_wait_ms: f64,
    pub response_read_ms: f64,
    pub validation_ms: f64,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub repair_reason: Option<String>,
    pub stage: Option<String>,
}

pub struct Completion {
    pub content: String,
    pub metrics: AttemptMetrics,
}

pub fn request_body(s: &Settings, messages: Value, output_budget: u32) -> Value {
    #[cfg(test)]
    let output_budget = s.test_output_budget.unwrap_or(output_budget);
    let mut body = json!({"model":s.model,"messages":messages,"stream":false});
    if s.json_mode {
        body["response_format"] = json!({"type":"json_object"});
    }
    // Do not send vendor-specific fields to arbitrary compatible services.
    match s.effective_protocol() {
        ThinkingProtocol::Deepseek => {
            body["thinking"] = json!({"type": if s.thinking { "enabled" } else { "disabled" }});
            if s.thinking {
                body["reasoning_effort"] = json!(s.reasoning_effort);
            }
            let reasoning_budget = if s.thinking {
                match s.reasoning_effort {
                    ReasoningEffort::Low => 8192,
                    ReasoningEffort::High => 16384,
                    ReasoningEffort::Max => 32768,
                }
            } else {
                0
            };
            body["max_tokens"] = json!(output_budget + reasoning_budget);
        }
        ThinkingProtocol::ReasoningEffort => {
            body["reasoning_effort"] = if s.thinking {
                json!(s.reasoning_effort)
            } else {
                json!("none")
            };
        }
        ThinkingProtocol::Auto | ThinkingProtocol::Omit => {}
    }
    // Thinking endpoints may reject or ignore temperature. Preserve the user's
    // saved value and apply it again when thinking is disabled.
    if !s.thinking_active() {
        body["temperature"] = json!(s.temperature);
    }
    body
}

pub async fn completion(
    client: &reqwest::Client,
    s: &Settings,
    key: &str,
    messages: Value,
    output_budget: u32,
) -> Result<Completion, String> {
    let started = Instant::now();
    let mut req = client
        .post(&s.endpoint)
        .json(&request_body(s, messages, output_budget));
    if !key.is_empty() {
        req = req.bearer_auth(key);
    }
    let mut response = req.send().await.map_err(|e| {
        if e.is_timeout() {
            "请求超时，请稍后重试。"
        } else {
            "网络连接失败，请检查接口地址和网络。"
        }
    })?;
    let response_wait_ms = started.elapsed().as_secs_f64() * 1000.0;
    let code = response.status().as_u16();
    if !response.status().is_success() {
        return Err(match code {
            401 | 403 => "鉴权失败，请检查 API Key 和权限。".into(),
            429 => "请求受限或额度不足，请稍后重试。".into(),
            404 => "接口或模型不存在，请检查完整端点与模型 ID。".into(),
            400 => "服务拒绝请求，请检查模型 ID、JSON 模式及温度/思考参数兼容性。可将思考参数格式切换为“不发送”后重试。".into(),
            _ => format!("模型服务请求失败（HTTP {code}）。"),
        });
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "读取模型响应失败或超时。")?
    {
        if bytes.len() + chunk.len() > 1_048_576 {
            return Err("模型响应过大，请缩短输入。".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| "服务未返回有效 JSON。")?;
    let choice = &value["choices"][0];
    if choice["finish_reason"] == "length" {
        return Err("模型输出被截断，请缩短输入，或降低推理强度后重试。".into());
    }
    let content = choice["message"]["content"]
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .ok_or("模型没有返回文本内容。")?
        .to_owned();
    let request_ms = started.elapsed().as_secs_f64() * 1000.0;
    Ok(Completion {
        content,
        metrics: AttemptMetrics {
            request_ms,
            response_wait_ms,
            response_read_ms: request_ms - response_wait_ms,
            input_tokens: value["usage"]["prompt_tokens"].as_u64(),
            output_tokens: value["usage"]["completion_tokens"].as_u64(),
            reasoning_tokens: value["usage"]["completion_tokens_details"]["reasoning_tokens"]
                .as_u64(),
            cached_input_tokens: value["usage"]["prompt_cache_hit_tokens"]
                .as_u64()
                .or_else(|| value["usage"]["prompt_tokens_details"]["cached_tokens"].as_u64()),
            ..AttemptMetrics::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        time::Duration,
    };

    fn deepseek() -> Settings {
        Settings {
            endpoint: "https://api.deepseek.com/v1/chat/completions".into(),
            model: "deepseek-v4-flash".into(),
            ..Settings::default()
        }
    }

    #[test]
    fn disables_default_thinking_and_sends_temperature() {
        let body = request_body(&deepseek(), json!([]), 2048);
        assert_eq!(body["thinking"]["type"], "disabled");
        assert_eq!(body["temperature"], 0.3);
        assert_eq!(body["max_tokens"], 2048);
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn thinking_uses_selected_effort_and_omits_temperature() {
        let mut s = deepseek();
        s.thinking = true;
        for (effort, name) in [
            (ReasoningEffort::Low, "low"),
            (ReasoningEffort::High, "high"),
            (ReasoningEffort::Max, "max"),
        ] {
            s.reasoning_effort = effort;
            let body = request_body(&s, json!([]), 2048);
            assert_eq!(body["thinking"]["type"], "enabled");
            assert_eq!(body["reasoning_effort"], name);
            assert!(body.get("temperature").is_none());
            assert!(body["max_tokens"].as_u64().unwrap() > 2048);
        }
    }

    #[test]
    fn does_not_guess_vendor_fields_for_other_endpoints() {
        let mut s = deepseek();
        s.endpoint = "https://example.com/v1/chat/completions".into();
        let body = request_body(&s, json!([]), 2048);
        assert!(body.get("thinking").is_none());
        assert!(body.get("reasoning_effort").is_none());
        assert!(body.get("max_tokens").is_none());
        s.thinking_protocol = ThinkingProtocol::ReasoningEffort;
        assert_eq!(
            request_body(&s, json!([]), 2048)["reasoning_effort"],
            "none"
        );
        s.thinking = true;
        s.reasoning_effort = ReasoningEffort::Low;
        assert_eq!(request_body(&s, json!([]), 2048)["reasoning_effort"], "low");
    }

    fn run_mock(status: &str, body: &str) -> Result<Completion, String> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!(
            "http://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
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
                    let size: usize = headers
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .unwrap()
                        .trim()
                        .parse()
                        .unwrap();
                    if bytes.len() >= end + 4 + size {
                        let sent: Value =
                            serde_json::from_slice(&bytes[end + 4..end + 4 + size]).unwrap();
                        assert_eq!(sent["temperature"], 0.7);
                        assert_eq!(sent["thinking"]["type"], "disabled");
                        break;
                    }
                }
            }
            stream.write_all(response.as_bytes()).unwrap();
        });
        let s = Settings {
            endpoint,
            model: "mock".into(),
            temperature: 0.7,
            thinking_protocol: ThinkingProtocol::Deepseek,
            ..Settings::default()
        };
        let result = tauri::async_runtime::block_on(completion(
            &reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            &s,
            "",
            json!([]),
            2048,
        ));
        server.join().unwrap();
        result
    }

    #[test]
    fn captures_usage_and_timing_without_retaining_reasoning_text() {
        let r = run_mock("200 OK", r#"{"choices":[{"message":{"content":"ok","reasoning_content":"not retained"},"finish_reason":"stop"}],"usage":{"prompt_tokens":100,"completion_tokens":70,"completion_tokens_details":{"reasoning_tokens":20}}}"#).unwrap();
        assert_eq!(r.content, "ok");
        assert_eq!(r.metrics.reasoning_tokens, Some(20));
        assert_eq!(r.metrics.input_tokens, Some(100));
        assert!(r.metrics.request_ms >= r.metrics.response_wait_ms);
        assert!(
            !serde_json::to_string(&r.metrics)
                .unwrap()
                .contains("not retained")
        );
    }

    #[test]
    fn handles_auth_truncation_and_invalid_responses() {
        assert!(
            run_mock("401 Unauthorized", "{}")
                .err()
                .unwrap()
                .contains("鉴权")
        );
        assert!(
            run_mock("200 OK", r#"{"choices":[{"finish_reason":"length"}]}"#)
                .err()
                .unwrap()
                .contains("截断")
        );
        assert!(run_mock("200 OK", "not JSON").is_err());
        assert!(run_mock("200 OK", r#"{"choices":[{"message":{"content":""}}]}"#).is_err());
    }
}
