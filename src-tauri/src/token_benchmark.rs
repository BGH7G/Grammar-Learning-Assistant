//! Paired public examples: identical targets, settings, budgets and validation.
use super::*;
use crate::benchmark::totals;
use serde_json::json;
use std::path::PathBuf;

const BEFORE: &str = include_str!(
    "../../docs/prompt-backups/before-token-optimization-20261026/src-tauri/src/prompts/layer-v2.txt"
);
const BBC: &str = "The British Broadcasting Corporation (BBC) is a British public service broadcaster headquartered at Broadcasting House in London, England.";
const STRIKE: &str = "His political views, disseminated to his 240m followers on X, strike many as plain-speaking and strike many more, including The Economist, as plainly bigoted.";
const QUOTE: &str = "Referencing the title of the film, Night Flower, Zeteo editor-in-chief Mehdi Hasan says AIPAC “blossoms in the dark, shrivels in the light. We’re trying to put some sunlight onto AIPAC.”";

struct Case {
    id: &'static str,
    text: &'static str,
    fragment: &'static str,
    depth: u8,
    role: &'static str,
    kind: &'static str,
    ctype: &'static str,
    grammar: &'static [&'static str],
}
impl Case {
    fn root(id: &'static str, text: &'static str) -> Self {
        Self {
            id,
            text,
            fragment: text,
            depth: 0,
            role: "root",
            kind: "sentence",
            ctype: "none",
            grammar: &[],
        }
    }
}

fn words(text: &str) -> String {
    analysis::pieces(text)
        .iter()
        .filter(|p| p.text.chars().any(char::is_alphanumeric))
        .map(|p| p.text.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}
fn content(session: &layered::Session, node: &layered::Node) -> String {
    session.tokens[node.first..=node.last]
        .iter()
        .filter(|p| p.text.chars().any(char::is_alphanumeric))
        .map(|p| p.text.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}
fn quality(
    id: &str,
    layer: &layered::Layer,
    session: &layered::Session,
) -> Vec<(&'static str, bool)> {
    let span = |role: &str, text: &str| {
        layer
            .nodes
            .iter()
            .find(|n| n.role == role && content(session, n) == words(text))
    };
    let typed = |text: &str, tag: &str| {
        layer
            .nodes
            .iter()
            .any(|n| content(session, n) == words(text) && n.grammar.iter().any(|t| t == tag))
    };
    match id {
        "modal-root"=>vec![("pronoun subject",typed("They","pronoun")),("infinitive object",span("object","to adjust").is_some_and(|n|n.ctype=="infinitive"))],
        "modal-parts"|"depth-cap"=>vec![("modal type",typed("may","modal_verb")),("lexical type",typed("struggle","lexical_verb")),("depth cap",layer.nodes.iter().all(|n|n.depth<4||!n.expandable))],
        "auxiliary-parts"=>vec![("auxiliary type",typed("has","auxiliary_verb")),("lexical type",typed("finished","lexical_verb"))],
        "bbc-root"=>vec![("linking verb",typed("is","linking_verb")),("whole predicative NP",span("predicative","a British public service broadcaster headquartered at Broadcasting House in London, England.").is_some())],
        "bbc-np"=>vec![("core NP",span("head","a British public service broadcaster").is_some_and(|n|n.grammar.iter().any(|t|t=="noun_phrase"))),
            ("postmodifier",span("attribute","headquartered at Broadcasting House in London, England.").is_some_and(|n|["postmodifier","past_participial"].iter().all(|t|n.grammar.iter().any(|g|g==t))))],
        "bbc-core"=>vec![("article",span("determiner","a").is_some()),("compound modifier",span("attribute","public service").is_some()),("noun head",span("head","broadcaster").is_some())],
        "bbc-participle"=>vec![("participial verb",span("predicate","headquartered").is_some()),("not a noun phrase",layer.nodes.iter().all(|n|n.role!="determiner"))],
        "strike-root"=>vec![("predicate starts at strike",layer.nodes.iter().any(|n|n.role=="predicate" && session.tokens[n.first].text=="strike" && n.kind=="phrase"))],
        "strike-predicate"=>vec![("shared-subject phrases",layer.nodes.iter().filter(|n|n.role=="coordinate" && n.kind=="phrase").count()==2), ("and connector",span("conjunction","and").is_some())],
        "strike-part"=>vec![("lexical predicate",span("predicate","strike").is_some()),("including stays in object",span("object","many more, including The Economist,").is_some()),("as complement",span("complement","as plainly bigoted.").is_some())],
        "quote-root"=>vec![("reported content stays together",layer.nodes.iter().any(|n|n.role=="object"&&n.ctype=="noun"&&content(session,n).starts_with("aipac blossoms")&&content(session,n).contains("we’re trying")))],
        "relative-root"=>vec![("subject retains relative",span("subject","The report that she wrote").is_some()),("main verb",span("predicate","surprised").is_some())],
        "relative-child"=>vec![("relative object",span("object","that").is_some()),("clause subject",span("subject","she").is_some()),("clause verb",span("predicate","wrote").is_some())],
        "nonfinite-root"=>vec![("purpose infinitive",layer.nodes.iter().any(|n|n.role=="adverbial"&&n.ctype=="infinitive")),("while clause",layer.nodes.iter().any(|n|n.role=="adverbial"&&n.ctype=="adverbial"))],
        "coordinate-root"=>vec![("independent clauses",layer.nodes.iter().filter(|n|n.role=="coordinate"&&n.kind=="clause").count()==2)],
        "existential-root"=>vec![("reporting object",layer.nodes.iter().any(|n|n.role=="object"&&n.ctype=="noun"))],
        "adjective-phrase"=>vec![("adjective head",span("head","proud").is_some()),("PP complement",span("complement","of her daughter.").is_some()),("not noun postmodifier",layer.nodes.iter().all(|n|!n.grammar.iter().any(|t|matches!(t.as_str(),"noun_phrase"|"postmodifier"))))],
        _=>unreachable!(),
    }
}

#[test]
#[ignore = "live paired token comparison; uses saved settings and public samples"]
fn compact_token_comparison() {
    let s: Settings = serde_json::from_slice(
        &fs::read(
            PathBuf::from(std::env::var("APPDATA").unwrap())
                .join("com.example.grammarassistant/model.json"),
        )
        .unwrap(),
    )
    .unwrap();
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
        "../docs/benchmarks/compact-{}-{stamp}",
        layered::VERSION
    ));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("before-prompt.txt"), BEFORE).unwrap();
    fs::write(
        dir.join("after-prompt.txt"),
        include_str!("prompts/layer-v2.txt"),
    )
    .unwrap();
    let mut cases = vec![
        Case::root("modal-root", "They may struggle to adjust."),
        Case::root("bbc-root", BBC),
        Case::root("strike-root", STRIKE),
        Case::root("quote-root", QUOTE),
        Case::root(
            "relative-root",
            "The report that she wrote surprised everyone.",
        ),
        Case::root(
            "nonfinite-root",
            "To finish the project on time, Maya worked late while her team reviewed the final report.",
        ),
        Case::root(
            "coordinate-root",
            "The report surprised me, and the editor disagreed.",
        ),
        Case::root(
            "existential-root",
            "I think there must be moments, though, when we need to rest and think.",
        ),
        Case {
            id: "modal-parts",
            text: "They may struggle to adjust.",
            fragment: "may struggle",
            depth: 1,
            role: "predicate",
            kind: "phrase",
            ctype: "none",
            grammar: &["verb_phrase"],
        },
        Case {
            id: "auxiliary-parts",
            text: "She has finished the work.",
            fragment: "has finished",
            depth: 1,
            role: "predicate",
            kind: "phrase",
            ctype: "none",
            grammar: &["verb_phrase"],
        },
        Case {
            id: "bbc-np",
            text: BBC,
            fragment: "a British public service broadcaster headquartered at Broadcasting House in London, England.",
            depth: 1,
            role: "predicative",
            kind: "phrase",
            ctype: "none",
            grammar: &["noun_phrase"],
        },
        Case {
            id: "bbc-core",
            text: BBC,
            fragment: "a British public service broadcaster",
            depth: 2,
            role: "head",
            kind: "phrase",
            ctype: "none",
            grammar: &["noun_phrase"],
        },
        Case {
            id: "bbc-participle",
            text: BBC,
            fragment: "headquartered at Broadcasting House in London, England.",
            depth: 2,
            role: "attribute",
            kind: "clause",
            ctype: "participial",
            grammar: &["postmodifier", "past_participial"],
        },
        Case {
            id: "strike-predicate",
            text: STRIKE,
            fragment: "strike many as plain-speaking and strike many more, including The Economist, as plainly bigoted.",
            depth: 1,
            role: "predicate",
            kind: "phrase",
            ctype: "none",
            grammar: &["verb_phrase"],
        },
        Case {
            id: "strike-part",
            text: STRIKE,
            fragment: "strike many more, including The Economist, as plainly bigoted.",
            depth: 2,
            role: "coordinate",
            kind: "phrase",
            ctype: "none",
            grammar: &["verb_phrase"],
        },
        Case {
            id: "relative-child",
            text: "The report that she wrote surprised everyone.",
            fragment: "that she wrote",
            depth: 1,
            role: "attribute",
            kind: "clause",
            ctype: "relative",
            grammar: &["postmodifier"],
        },
        Case {
            id: "adjective-phrase",
            text: "She is very proud of her daughter.",
            fragment: "very proud of her daughter.",
            depth: 1,
            role: "predicative",
            kind: "phrase",
            ctype: "none",
            grammar: &["adjective_phrase"],
        },
        Case {
            id: "depth-cap",
            text: "They may struggle to adjust.",
            fragment: "may struggle",
            depth: 3,
            role: "predicate",
            kind: "phrase",
            ctype: "none",
            grammar: &["verb_phrase"],
        },
    ];
    if let Ok(selection) = std::env::var("GRAMMAR_TOKEN_CASES") {
        let ids: Vec<_> = selection.split(',').collect();
        cases.retain(|case| ids.contains(&case.id));
        assert!(!cases.is_empty());
    }
    let rounds = std::env::var("GRAMMAR_TOKEN_ROUNDS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(2);
    assert!((1..=2).contains(&rounds));
    let mut report = json!({"beforeVersion":"layer-v2.9.2-grammar-types","afterVersion":layered::VERSION,"model":s.model,"temperature":s.temperature,"thinking":s.thinking,"reasoningEffort":s.reasoning_effort,"jsonMode":s.json_mode,"rounds":rounds,"caseCount":cases.len(),"complete":false,"rows":[]});
    let path = dir.join("report.json");
    println!("COMPACT_REPORT={}", path.display());
    let mut failures = Vec::new();
    for round in 0..rounds {
        for (i, case) in cases.iter().enumerate() {
            let order = if (round + i) % 2 == 0 {
                ["before", "after"]
            } else {
                ["after", "before"]
            };
            for condition in order {
                let session = layered::Session::prepare(
                    format!("{round}-{}-{condition}", case.id),
                    case.text,
                    "paired".into(),
                    false,
                )
                .unwrap();
                let fragment: Vec<_> = analysis::pieces(case.fragment)
                    .into_iter()
                    .filter(|p| !p.text.chars().all(char::is_whitespace))
                    .map(|p| p.text)
                    .collect();
                let start = session
                    .tokens
                    .windows(fragment.len())
                    .position(|w| w.iter().zip(&fragment).all(|(t, f)| t.text == *f))
                    .unwrap();
                let mut parent = session.roots[0].clone();
                parent.first = start;
                parent.last = start + fragment.len() - 1;
                parent.depth = case.depth;
                parent.role = case.role.into();
                parent.kind = case.kind.into();
                parent.ctype = case.ctype.into();
                parent.grammar = case.grammar.iter().map(|s| s.to_string()).collect();
                if case.depth > 0 {
                    parent.id = format!("{}:target", parent.id);
                }
                let result = if condition == "before" {
                    tauri::async_runtime::block_on(layered::query_token_baseline(
                        &client, &s, &key, &session, &parent, BEFORE,
                    ))
                } else {
                    tauri::async_runtime::block_on(layered::query(
                        &client, &s, &key, &session, &parent,
                    ))
                };
                let row = match result {
                    Ok((layer, metrics)) => {
                        let checks = quality(case.id, &layer, &session);
                        let passed = checks.iter().all(|(_, passed)| *passed);
                        if condition == "after" && !passed {
                            failures.push(format!(
                                "round{} {}: {:?}",
                                round + 1,
                                case.id,
                                checks.iter().filter(|(_, v)| !*v).collect::<Vec<_>>()
                            ));
                        }
                        json!({"round":round+1,"case":case.id,"condition":condition,"text":case.text,"targetText":case.fragment,"parent":parent,"status":"ok","qualityPassed":passed,"checks":checks,"layer":layer,"totals":totals(std::slice::from_ref(&metrics)),"metrics":metrics})
                    }
                    Err(e) => {
                        if condition == "after" {
                            failures.push(format!("round{} {}: {}", round + 1, case.id, e.message));
                        }
                        json!({"round":round+1,"case":case.id,"condition":condition,"text":case.text,"targetText":case.fragment,"parent":parent,"status":"error","qualityPassed":false,"error":e.message,"totals":totals(std::slice::from_ref(&e.metrics)),"metrics":e.metrics})
                    }
                };
                println!(
                    "COMPACT round={} case={} condition={condition} status={} quality={} tokens={}",
                    round + 1,
                    case.id,
                    row["status"],
                    row["qualityPassed"],
                    row["totals"]
                );
                report["rows"].as_array_mut().unwrap().push(row);
                fs::write(&path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
            }
        }
    }
    report["complete"] = json!(true);
    report["afterFailures"] = json!(failures);
    fs::write(&path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    assert!(failures.is_empty(), "{}", failures.join("; "));
}
