//! Explicit opt-in, same-input comparison. No keys or private input are written.
use super::*;
use serde_json::{Value, json};
use std::{collections::VecDeque, path::PathBuf};

const SAMPLES: &[(&str, &str)] = &[
    ("simple", "The cat sleeps."),
    ("relative", "The cat that I saw yesterday sat on the mat."),
    ("coordination", "I wanted to leave, but the train was late."),
    (
        "nonfinite",
        "To finish the project on time, Maya worked late while her team reviewed the final report.",
    ),
    (
        "long",
        "Sweeping restrictions have been imposed on demonstrations across France but hundreds of thousands of protesters are still expected to gather in Paris and elsewhere for a second round of protests organised by students, trade unions and wider anti-government groups.",
    ),
    (
        "attachment",
        "The report published by the committee yesterday suggests that the measures introduced last year have reduced pollution.",
    ),
];

fn totals(metrics: &[AnalysisMetrics]) -> Value {
    let attempts: Vec<_> = metrics.iter().flat_map(|m| m.attempts.iter()).collect();
    fn sum(values: impl Iterator<Item = Option<u64>>) -> Option<u64> {
        values.collect::<Option<Vec<_>>>().map(|v| v.iter().sum())
    }
    json!({
        "requests":attempts.len(),
        "inputTokens":sum(attempts.iter().map(|m|m.input_tokens)),
        "outputTokens":sum(attempts.iter().map(|m|m.output_tokens)),
        "reasoningTokens":sum(attempts.iter().map(|m|m.reasoning_tokens)),
        "cachedInputTokens":sum(attempts.iter().map(|m|m.cached_input_tokens)),
        "httpMs":attempts.iter().map(|m|m.request_ms).sum::<f64>(),
        "retryRequests":metrics.iter().map(|m|m.attempts.len().saturating_sub(1)).sum::<usize>()
    })
}

fn save(path: &PathBuf, report: &Value) {
    fs::write(path, serde_json::to_vec_pretty(report).unwrap()).unwrap();
}

#[test]
#[ignore = "calls two public regression cases to diagnose enum normalization"]
fn probe_v2_validation() {
    let config = PathBuf::from(std::env::var("APPDATA").unwrap())
        .join("com.example.grammarassistant/model.json");
    let mut s: Settings = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
    s.thinking = false;
    s.temperature = 0.3;
    s.test_output_budget = Some(8192);
    let key = credential(&s).unwrap().get_password().unwrap();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    for index in [1, 3, 4] {
        let (id, text) = SAMPLES[index];
        let session =
            layered::Session::prepare(format!("probe-{id}"), text, "probe".into(), false).unwrap();
        let metrics = match tauri::async_runtime::block_on(layered::query(
            &client,
            &s,
            &key,
            &session,
            &session.roots[0],
        )) {
            Ok((_, m)) => m,
            Err(e) => e.metrics,
        };
        println!("{id}: {}", serde_json::to_string(&metrics).unwrap());
    }
}

#[test]
#[ignore = "calls the configured service for paired v1/v2 token comparisons"]
fn benchmark_v1_v2() {
    assert_eq!(
        include_str!("prompts/analyze.txt"),
        include_str!("../../docs/prompt-backups/v1-20261024-165343/prompts/analyze.txt"),
        "Baseline prompt no longer matches its backup"
    );
    let config = PathBuf::from(std::env::var("APPDATA").unwrap())
        .join("com.example.grammarassistant/model.json");
    let mut s: Settings = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
    s.thinking = false;
    s.temperature = 0.3;
    s.test_output_budget = Some(8192);
    validate_settings(&s).unwrap();
    assert!(
        s.effective_protocol() == settings::ThinkingProtocol::Deepseek,
        "Benchmark requires explicit non-thinking control for this service"
    );
    let key = credential(&s).unwrap().get_password().unwrap();
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../docs/benchmarks/v1-v2-{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("report.json");
    let mut report = json!({"model":s.model,"endpointHost":reqwest::Url::parse(&s.endpoint).unwrap().host_str(),"temperature":s.temperature,"thinking":false,"maxOutputTokens":8192,"jsonMode":s.json_mode,"rounds":2,"samples":SAMPLES,"backup":"docs/prompt-backups/v1-20261024-165343","v2Prompt":layered::VERSION,"maxDepth":layered::MAX_DEPTH,"complete":false,"rows":[]});
    save(&path, &report);
    println!("BENCHMARK_REPORT={}", path.display());
    for round in 0..2 {
        for (sample_id, text) in SAMPLES {
            // Alternate condition order to reduce systematic warmup/order bias.
            for condition in if round % 2 == 0 {
                ["v1_full", "v2"]
            } else {
                ["v2", "v1_full"]
            } {
                let started = Instant::now();
                if condition == "v1_full" {
                    let row = match tauri::async_runtime::block_on(analyzer::analyze(
                        &client, &s, &key, text,
                    )) {
                        Ok(result) => {
                            json!({"round":round+1,"sample":sample_id,"condition":"v1_full","status":"ok","elapsedMs":started.elapsed().as_secs_f64()*1000.0,"totals":totals(&[result.metrics]),"analysis":result.analysis})
                        }
                        Err(error) => {
                            json!({"round":round+1,"sample":sample_id,"condition":"v1_full","status":"error","elapsedMs":started.elapsed().as_secs_f64()*1000.0,"error":error,"totals":null})
                        }
                    };
                    println!(
                        "round={} sample={} condition=v1_full status={} totals={}",
                        round + 1,
                        sample_id,
                        row["status"],
                        row["totals"]
                    );
                    report["rows"].as_array_mut().unwrap().push(row);
                    save(&path, &report);
                } else {
                    let id = format!("bench-{round}-{sample_id}");
                    let mut session =
                        layered::Session::prepare(id, text, "benchmark-fixed-config".into(), false)
                            .unwrap();
                    let root = session.roots[0].clone();
                    match tauri::async_runtime::block_on(layered::query(
                        &client, &s, &key, &session, &root,
                    )) {
                        Err(error) => {
                            println!(
                                "round={} sample={} condition=v2_first status=error",
                                round + 1,
                                sample_id
                            );
                            report["rows"].as_array_mut().unwrap().push(json!({"round":round+1,"sample":sample_id,"condition":"v2_first","status":"error","elapsedMs":started.elapsed().as_secs_f64()*1000.0,"error":error.message,"totals":totals(std::slice::from_ref(&error.metrics)),"metrics":error.metrics}));
                            save(&path, &report);
                        }
                        Ok((first, metrics)) => {
                            let first_row = json!({"round":round+1,"sample":sample_id,"condition":"v2_first","status":"ok","elapsedMs":started.elapsed().as_secs_f64()*1000.0,"totals":totals(std::slice::from_ref(&metrics)),"metrics":metrics,"layer":first});
                            println!(
                                "round={} sample={} condition=v2_first status=ok totals={}",
                                round + 1,
                                sample_id,
                                first_row["totals"]
                            );
                            report["rows"].as_array_mut().unwrap().push(first_row);
                            save(&path, &report);
                            let mut queue: VecDeque<_> = first
                                .nodes
                                .iter()
                                .filter(|n| n.expandable)
                                .cloned()
                                .collect();
                            session.attach(first);
                            let mut all_metrics = vec![metrics];
                            let mut failures = Vec::new();
                            while let Some(node) = queue.pop_front() {
                                if session.layers.len() >= 48 {
                                    failures.push(json!({"message":"Expansion budget reached"}));
                                    break;
                                }
                                match tauri::async_runtime::block_on(layered::query(
                                    &client, &s, &key, &session, &node,
                                )) {
                                    Ok((layer, m)) => {
                                        queue.extend(
                                            layer.nodes.iter().filter(|n| n.expandable).cloned(),
                                        );
                                        all_metrics.push(m);
                                        session.attach(layer);
                                    }
                                    Err(error) => {
                                        failures.push(json!({"message":error.message,"target":[node.first,node.last],"depth":node.depth+1}));
                                        all_metrics.push(error.metrics);
                                    }
                                }
                            }
                            let mut store = layered::Store::default();
                            let session_id = session.id.clone();
                            store.insert(session.clone());
                            let cache_hit = matches!(
                                store.plan(&session_id, &root.id, "benchmark-fixed-config"),
                                Ok(layered::Plan::Cached(_))
                            );
                            let row = json!({"round":round+1,"sample":sample_id,"condition":"v2_expanded_to_depth4","status":if failures.is_empty(){"ok"}else{"incomplete"},"elapsedMs":started.elapsed().as_secs_f64()*1000.0,"totals":totals(&all_metrics),"errors":failures,"failedCallsHaveUnknownTokens":all_metrics.iter().flat_map(|m|m.attempts.iter()).any(|a|a.input_tokens.is_none()||a.output_tokens.is_none()),"layers":session.layers.values().collect::<Vec<_>>(),"cacheHitVerified":cache_hit});
                            println!(
                                "round={} sample={} condition=v2_expanded_to_depth4 status={} totals={}",
                                round + 1,
                                sample_id,
                                row["status"],
                                row["totals"]
                            );
                            report["rows"].as_array_mut().unwrap().push(row);
                            save(&path, &report);
                        }
                    }
                }
            }
        }
    }
    report["complete"] = json!(true);
    save(&path, &report);
}

#[test]
#[ignore = "live public quotation regression; stores no credentials"]
fn quotation_prompt_probe() {
    let config = PathBuf::from(std::env::var("APPDATA").unwrap())
        .join("com.example.grammarassistant/model.json");
    let s: Settings = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
    let key = credential(&s).unwrap().get_password().unwrap();
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let text = "Referencing the title of the film, Night Flower, Zeteo editor-in-chief Mehdi Hasan says AIPAC “blossoms in the dark, shrivels in the light. We’re trying to put some sunlight onto AIPAC.”";
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../docs/benchmarks/quotation-{}", layered::VERSION));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("report.json");
    let mut report = json!({"promptVersion":layered::VERSION,"model":s.model,"temperature":s.temperature,"thinking":s.thinking,"reasoningEffort":s.reasoning_effort,"text":text,"rows":[]});
    fs::write(dir.join("prompt.txt"), include_str!("prompts/layer-v2.txt")).unwrap();
    for split in [true, false] {
        let mut session =
            layered::Session::prepare(format!("quote-{split}"), text, "probe".into(), split)
                .unwrap();
        let mut queue: VecDeque<_> = session.roots.clone().into();
        let mut count = 0;
        while let Some(parent) = queue.pop_front() {
            if count >= 10 {
                break;
            }
            count += 1;
            let started = Instant::now();
            let result = tauri::async_runtime::block_on(layered::query(
                &client, &s, &key, &session, &parent,
            ));
            let row = match result {
                Ok((layer, metrics)) => {
                    for n in &layer.nodes {
                        if n.expandable
                            && (n.role == "object"
                                || n.role == "predicate"
                                || n.role == "coordinate"
                                || n.role == "main_clause")
                        {
                            queue.push_back(n.clone());
                        }
                    }
                    let row = json!({"split":split,"parent":parent,"elapsedMs":started.elapsed().as_secs_f64()*1000.0,"status":"ok","layer":layer,"totals":totals(std::slice::from_ref(&metrics)),"metrics":metrics});
                    session.attach(layer);
                    row
                }
                Err(e) => {
                    json!({"split":split,"parent":parent,"status":"error","error":e.message,"totals":totals(std::slice::from_ref(&e.metrics))})
                }
            };
            println!(
                "QUOTE split={split} parent={} status={} tokens={}",
                parent.id, row["status"], row["totals"]
            );
            report["rows"].as_array_mut().unwrap().push(row);
            save(&path, &report);
        }
        fs::write(
            dir.join(format!("document-split-{split}.json")),
            serde_json::to_vec_pretty(&session.document()).unwrap(),
        )
        .unwrap();
    }
    println!("QUOTE_REPORT={}", path.display());
}

#[test]
#[ignore = "live strike-as regression; uses public examples and the saved model parameters"]
fn strike_boundary_probe() {
    let config = PathBuf::from(std::env::var("APPDATA").unwrap())
        .join("com.example.grammarassistant/model.json");
    let s: Settings = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
    let key = credential(&s).unwrap().get_password().unwrap();
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../docs/benchmarks/strike-{}-{stamp}",
        layered::VERSION
    ));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("prompt.txt"), include_str!("prompts/layer-v2.txt")).unwrap();
    let path = dir.join("report.json");
    let mut report = json!({"promptVersion":layered::VERSION,"model":s.model,"temperature":s.temperature,"thinking":s.thinking,"reasoningEffort":s.reasoning_effort,"complete":false,"rows":[]});
    println!("STRIKE_REPORT={}", path.display());
    let samples = [
        (
            "original-1",
            "His political views, disseminated to his 240m followers on X, strike many as plain-speaking and strike many more, including The Economist, as plainly bigoted.",
            true,
        ),
        (
            "original-2",
            "His political views, disseminated to his 240m followers on X, strike many as plain-speaking and strike many more, including The Economist, as plainly bigoted.",
            true,
        ),
        (
            "transfer",
            "Her proposals, circulated among the staff, strike some as sensible and strike others, including the manager, as impractical.",
            true,
        ),
        (
            "finite-control",
            "His team disseminated the report and contacted the editor.",
            false,
        ),
        (
            "clause-control",
            "The report surprised me, and the editor disagreed.",
            false,
        ),
    ];
    for (id, text, deep) in samples {
        let mut session =
            layered::Session::prepare(id.into(), text, "probe".into(), false).unwrap();
        let mut queue: VecDeque<_> = session.roots.clone().into();
        let mut count = 0;
        while let Some(parent) = queue.pop_front() {
            if count >= 9 {
                break;
            }
            count += 1;
            let result = tauri::async_runtime::block_on(layered::query(
                &client, &s, &key, &session, &parent,
            ));
            let row = match result {
                Ok((layer, metrics)) => {
                    if deep {
                        for n in &layer.nodes {
                            let content = session.tokens[n.first..=n.last]
                                .iter()
                                .map(|t| t.text.as_str())
                                .collect::<Vec<_>>()
                                .join(" ");
                            if n.expandable
                                && (n.role == "predicate"
                                    || n.role == "coordinate"
                                    || n.role == "main_clause"
                                    || n.role == "attribute"
                                    || n.role == "subject"
                                    || n.role == "object" && content.contains("including"))
                            {
                                queue.push_back(n.clone());
                            }
                        }
                    }
                    let row = json!({"sample":id,"text":text,"parent":parent,"status":"ok","layer":layer,"totals":totals(std::slice::from_ref(&metrics)),"metrics":metrics});
                    session.attach(layer);
                    row
                }
                Err(e) => {
                    json!({"sample":id,"text":text,"parent":parent,"status":"error","error":e.message,"totals":totals(std::slice::from_ref(&e.metrics))})
                }
            };
            println!(
                "STRIKE sample={id} parent={} status={} totals={}",
                parent.id, row["status"], row["totals"]
            );
            report["rows"].as_array_mut().unwrap().push(row);
            save(&path, &report);
        }
        fs::write(
            dir.join(format!("{id}.json")),
            serde_json::to_vec_pretty(&session.document()).unwrap(),
        )
        .unwrap();
    }
    report["complete"] = json!(true);
    save(&path, &report);
}

#[test]
#[ignore = "live NP teaching regression; uses public examples and saved model settings"]
fn noun_phrase_teaching_probe() {
    let config = PathBuf::from(std::env::var("APPDATA").unwrap())
        .join("com.example.grammarassistant/model.json");
    let s: Settings = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
    let key = credential(&s).unwrap().get_password().unwrap();
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../docs/benchmarks/noun-phrase-{}-{stamp}",
        layered::VERSION
    ));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("prompt.txt"), include_str!("prompts/layer-v2.txt")).unwrap();
    let path = dir.join("report.json");
    let mut report = json!({"promptVersion":layered::VERSION,"model":s.model,"temperature":s.temperature,"thinking":s.thinking,"complete":false,"rows":[],"checks":[]});
    println!("NP_REPORT={}", path.display());
    let original = "The British Broadcasting Corporation (BBC) is a British public service broadcaster headquartered at Broadcasting House in London, England.";
    let samples = [
        ("bbc-1", original, "predicative", "past_participial"),
        ("bbc-2", original, "predicative", "past_participial"),
        (
            "irregular",
            "We visited an old stone bridge built across the river.",
            "object",
            "past_participial",
        ),
        (
            "present",
            "She greeted the young scientist working in the laboratory.",
            "object",
            "present_participial",
        ),
        (
            "adjective-control",
            "She is very proud of her daughter.",
            "predicative",
            "",
        ),
    ];
    let mut failures = Vec::new();
    for (id, text, focus_role, participle) in samples {
        let mut session =
            layered::Session::prepare(id.into(), text, "probe".into(), false).unwrap();
        let mut queue: VecDeque<_> = session.roots.clone().into();
        let mut count = 0;
        while let Some(parent) = queue.pop_front() {
            if count >= 4 {
                break;
            }
            count += 1;
            let result = tauri::async_runtime::block_on(layered::query(
                &client, &s, &key, &session, &parent,
            ));
            let row = match result {
                Ok((layer, metrics)) => {
                    for n in &layer.nodes {
                        if n.expandable
                            && ((parent.depth == 0 && n.role == focus_role)
                                || (parent.depth == 1
                                    && (n.grammar.iter().any(|t| t == "noun_phrase")
                                        || n.grammar.iter().any(|t| t == "postmodifier"))))
                        {
                            queue.push_back(n.clone());
                        }
                    }
                    let row = json!({"sample":id,"text":text,"parent":parent,"status":"ok","layer":layer,"totals":totals(std::slice::from_ref(&metrics)),"metrics":metrics});
                    session.attach(layer);
                    row
                }
                Err(e) => {
                    failures.push(format!("{id}: {}", e.message));
                    json!({"sample":id,"text":text,"parent":parent,"status":"error","error":e.message})
                }
            };
            println!(
                "NP sample={id} parent={} status={} totals={}",
                parent.id, row["status"], row["totals"]
            );
            report["rows"].as_array_mut().unwrap().push(row);
            save(&path, &report);
        }
        let focus = session
            .layers
            .get(&session.roots[0].id)
            .and_then(|l| l.nodes.iter().find(|n| n.role == focus_role));
        let focused = focus.and_then(|n| session.layers.get(&n.id));
        let passed = focused.is_some_and(|layer| {
            if participle.is_empty() {
                return layer.nodes.iter().all(|n| {
                    !n.grammar.iter().any(|t| {
                        matches!(t.as_str(), "noun_phrase" | "postmodifier" | "premodifier")
                    })
                });
            }
            let core = layer
                .nodes
                .iter()
                .find(|n| n.role == "head" && n.grammar.iter().any(|t| t == "noun_phrase"));
            let modifier = layer.nodes.iter().find(|n| {
                n.grammar.iter().any(|t| t == "postmodifier")
                    && n.grammar.iter().any(|t| t == participle)
            });
            core.is_some_and(|n| {
                session
                    .layers
                    .get(&n.id)
                    .is_some_and(|l| l.nodes.iter().any(|n| n.role == "determiner"))
            }) && modifier.is_some()
                && layer.nodes.len() == 2
        });
        if !passed {
            failures.push(format!("{id}: teaching hierarchy check failed"));
        }
        report["checks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"sample":id,"passed":passed}));
        fs::write(
            dir.join(format!("{id}.json")),
            serde_json::to_vec_pretty(&session.document()).unwrap(),
        )
        .unwrap();
    }
    report["complete"] = json!(true);
    report["failures"] = json!(failures);
    save(&path, &report);
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

#[test]
#[ignore = "live word-class regression; uses public examples and saved model settings"]
fn grammar_type_teaching_probe() {
    let config = PathBuf::from(std::env::var("APPDATA").unwrap())
        .join("com.example.grammarassistant/model.json");
    let s: Settings = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
    let key = credential(&s).unwrap().get_password().unwrap();
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../docs/benchmarks/grammar-types-{}-{stamp}",
        layered::VERSION
    ));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("prompt.txt"), include_str!("prompts/layer-v2.txt")).unwrap();
    let path = dir.join("report.json");
    let mut report = json!({"promptVersion":layered::VERSION,"model":s.model,"temperature":s.temperature,"thinking":s.thinking,"complete":false,"rows":[],"checks":[]});
    println!("GRAMMAR_REPORT={}", path.display());
    type GrammarSample<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)]);
    let samples: &[GrammarSample<'_>] = &[
        (
            "modal",
            "They may struggle to adjust.",
            &[
                ("They", "pronoun"),
                ("may", "modal_verb"),
                ("struggle", "lexical_verb"),
            ],
        ),
        (
            "auxiliary",
            "She has finished the work.",
            &[("has", "auxiliary_verb"), ("finished", "lexical_verb")],
        ),
        (
            "lexical-have",
            "She has a bicycle.",
            &[("has", "lexical_verb")],
        ),
        (
            "linking",
            "She is happy.",
            &[("is", "linking_verb"), ("happy", "adjective")],
        ),
        (
            "other-classes",
            "The young scientist worked quietly in London.",
            &[
                ("young", "adjective"),
                ("scientist", "noun"),
                ("quietly", "adverb"),
                ("in", "preposition"),
                ("London", "proper_noun"),
            ],
        ),
        (
            "noun-can",
            "The can fell.",
            &[("can", "noun"), ("fell", "lexical_verb")],
        ),
        (
            "discourse",
            "Well, they may leave.",
            &[
                ("Well", "discourse_marker"),
                ("may", "modal_verb"),
                ("leave", "lexical_verb"),
            ],
        ),
    ];
    let mut failures = Vec::new();
    for &(id, text, expected) in samples {
        let mut session =
            layered::Session::prepare(id.into(), text, "probe".into(), false).unwrap();
        let mut queue: VecDeque<_> = session.roots.clone().into();
        let mut count = 0;
        while let Some(parent) = queue.pop_front() {
            if count >= 5 {
                break;
            }
            count += 1;
            let result = tauri::async_runtime::block_on(layered::query(
                &client, &s, &key, &session, &parent,
            ));
            let row = match result {
                Ok((layer, metrics)) => {
                    for n in &layer.nodes {
                        if n.expandable
                            && (n.role == "predicate"
                                || (id == "other-classes"
                                    && matches!(n.role.as_str(), "subject" | "adverbial"))
                                || id == "noun-can" && n.role == "subject")
                        {
                            queue.push_back(n.clone());
                        }
                    }
                    let row = json!({"sample":id,"text":text,"parent":parent,"status":"ok","layer":layer,"totals":totals(std::slice::from_ref(&metrics)),"metrics":metrics});
                    session.attach(layer);
                    row
                }
                Err(e) => {
                    failures.push(format!("{id}: {}", e.message));
                    json!({"sample":id,"status":"error","parent":parent,"error":e.message,"totals":totals(std::slice::from_ref(&e.metrics))})
                }
            };
            println!(
                "GRAMMAR sample={id} parent={} status={} totals={}",
                parent.id, row["status"], row["totals"]
            );
            report["rows"].as_array_mut().unwrap().push(row);
            save(&path, &report);
        }
        for &(word, tag) in expected {
            let passed = session.nodes.values().any(|n| {
                n.grammar.iter().any(|t| t == tag)
                    && session.tokens[n.first..=n.last]
                        .iter()
                        .filter(|t| t.text.chars().any(char::is_alphanumeric))
                        .map(|t| t.text.as_str())
                        .eq(std::iter::once(word))
            });
            if !passed {
                failures.push(format!("{id}: missing {word}/{tag}"));
            }
            report["checks"]
                .as_array_mut()
                .unwrap()
                .push(json!({"sample":id,"word":word,"tag":tag,"passed":passed}));
        }
        if id == "modal" {
            let passed = session
                .nodes
                .values()
                .any(|n| n.role == "object" && n.ctype == "infinitive");
            if !passed {
                failures.push("modal: infinitive object missing".into());
            }
            report["checks"]
                .as_array_mut()
                .unwrap()
                .push(json!({"sample":id,"tag":"infinitive object","passed":passed}));
        }
        // Include fetched child layers for faithful UI replay without more API calls.
        let mut document = serde_json::to_value(session.document()).unwrap();
        document["layers"] = json!(session.layers.values().collect::<Vec<_>>());
        fs::write(
            dir.join(format!("{id}.json")),
            serde_json::to_vec_pretty(&document).unwrap(),
        )
        .unwrap();
    }
    report["complete"] = json!(true);
    report["failures"] = json!(failures);
    save(&path, &report);
    assert!(failures.is_empty(), "{}", failures.join("; "));
}
