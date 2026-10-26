use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingProtocol {
    #[default]
    Auto,
    Deepseek,
    ReasoningEffort,
    Omit,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    Low,
    #[default]
    High,
    Max,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub endpoint: String,
    pub model: String,
    pub json_mode: bool,
    pub temperature: f64,
    pub thinking: bool,
    pub reasoning_effort: ReasoningEffort,
    pub thinking_protocol: ThinkingProtocol,
    #[cfg(test)]
    #[serde(skip)]
    pub test_output_budget: Option<u32>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            model: String::new(),
            json_mode: false,
            temperature: 0.3,
            thinking: false,
            reasoning_effort: ReasoningEffort::High,
            thinking_protocol: ThinkingProtocol::Auto,
            #[cfg(test)]
            test_output_budget: None,
        }
    }
}

impl Settings {
    pub fn effective_protocol(&self) -> ThinkingProtocol {
        match self.thinking_protocol {
            ThinkingProtocol::Auto => {
                if reqwest::Url::parse(&self.endpoint)
                    .ok()
                    .and_then(|url| url.host_str().map(str::to_owned))
                    .as_deref()
                    == Some("api.deepseek.com")
                {
                    ThinkingProtocol::Deepseek
                } else {
                    ThinkingProtocol::Omit
                }
            }
            protocol => protocol,
        }
    }

    pub fn thinking_active(&self) -> bool {
        self.thinking && self.effective_protocol() != ThinkingProtocol::Omit
    }
}

pub fn validate_settings(s: &Settings) -> Result<(), String> {
    let url = reqwest::Url::parse(&s.endpoint).map_err(|_| "请输入完整接口 URL。")?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if !(url.scheme() == "https" || (url.scheme() == "http" && local))
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.query().is_some()
    {
        return Err(
            "接口须使用 HTTPS（本机服务可用 HTTP），且不能包含账号、查询参数或片段。".into(),
        );
    }
    if s.model.trim().is_empty() {
        return Err("请填写模型 ID。".into());
    }
    if !s.temperature.is_finite() || !(0.0..=2.0).contains(&s.temperature) {
        return Err("温度须为 0–2 之间的数值。".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_old_configuration_without_losing_existing_fields() {
        let s: Settings = serde_json::from_str(r#"{"endpoint":"https://api.deepseek.com/v1/chat/completions","model":"deepseek-v4-flash","jsonMode":true}"#).unwrap();
        assert_eq!(s.model, "deepseek-v4-flash");
        assert!(s.json_mode);
        assert_eq!(s.temperature, 0.3);
        assert!(!s.thinking_active());
        assert!(s.effective_protocol() == ThinkingProtocol::Deepseek);
        let saved = serde_json::to_string(&s).unwrap();
        let loaded: Settings = serde_json::from_str(&saved).unwrap();
        assert_eq!(loaded.endpoint, s.endpoint);
    }

    #[test]
    fn validates_temperature_and_endpoint() {
        let mut s = Settings {
            endpoint: "https://example.com/v1/chat/completions".into(),
            model: "model".into(),
            ..Settings::default()
        };
        assert!(validate_settings(&s).is_ok());
        for temperature in [-0.1, 2.1, f64::NAN, f64::INFINITY] {
            s.temperature = temperature;
            assert!(validate_settings(&s).is_err());
        }
        s.temperature = 0.7;
        s.endpoint = "http://127.0.0.1:1234/v1/chat/completions".into();
        assert!(validate_settings(&s).is_ok());
        s.endpoint = "http://example.com/v1".into();
        assert!(validate_settings(&s).is_err());
        s.endpoint = "https://example.com/?key=secret".into();
        assert!(validate_settings(&s).is_err());
    }
}
