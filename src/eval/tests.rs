#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn temp() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "dutchman-pilot-eval-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    path
}
fn recording(message: Value) -> Recording {
    Recording {status:200, raw:json!({"choices":[{"message":message}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}).to_string(),error:None,latency_ms:5,tokens:Some(json!({"total_tokens":15})),hand_backs:None,retry_after:None}
}
fn command() -> Claim {
    Claim::Command {
        id: "solid.box".into(),
        args: json!({"length":20,"width":20,"height":20}),
    }
}
fn native(id: &str, args: Value) -> Recording {
    recording(
        json!({"role":"assistant","tool_calls":[{"type":"function","function":{"name":"execute","arguments":json!({"command":id,"params":args}).to_string()}}]}),
    )
}
pub(super) fn passing_result() -> CaseResult {
    let record = native("solid.box", json!({"length":20,"width":20,"height":20}));
    let turn = Turn {
        score: judge(&record, &command()),
        recording: record,
    };
    CaseResult {
        case: Case {
            id: "one".into(),
            utterance: "Make a 20 mm cube.".into(),
            state: "Empty design".into(),
            bucket: Bucket::Create,
            claim: command(),
        },
        cached: false,
        classification: Verdict::Pass,
        turns: vec![turn],
    }
}
#[test]
fn command_and_argument_correctness_and_paths() {
    let record = native(
        "solid.box",
        json!({"length":20.0,"width":20,"height":20,"corner":[0,0,0]}),
    );
    assert_eq!(judge(&record, &command()).verdict, Verdict::Pass);
    assert_eq!(judge(&record, &command()).path, TurnPath::NativeCall);
    let bad = native("solid.box", json!({"length":2,"width":20,"height":20}));
    let score = judge(&bad, &command());
    assert_eq!(score.command_correct, Some(true));
    assert_eq!(score.arguments_correct, Some(false));
    let bad = native("solid.sphere", json!({"radius":20}));
    assert_eq!(judge(&bad, &command()).command_correct, Some(false));
    for object in [
        json!({"tool":"execute","args":{"command":"solid.box","params":{"length":20,"width":20,"height":20}}}),
        json!({"function":{"name":"execute","arguments":{"command":"solid.box","params":{"length":20,"width":20,"height":20}}}}),
        json!({"solid.box":{"length":20,"width":20,"height":20}}),
        json!({"name":"solid.box","arguments":{"length":20,"width":20,"height":20}}),
    ] {
        let record = recording(json!({"content":format!("```json\n{object}\n```\n")}));
        assert_eq!(judge(&record, &command()).verdict, Verdict::Pass);
        assert_eq!(judge(&record, &command()).path, TurnPath::TextCall);
    }
    assert!(subset(
        &json!({"a":{"b":[1,2]}}),
        &json!({"a":{"b":[1.0,2],"c":3},"d":4})
    ));
    assert!(!subset(&json!([1]), &json!([1, 2])));
    assert!(!subset(&json!(20), &json!("20")));
}
#[test]
fn lexical_claims_refusals_and_no_action_from_prose() {
    for (text, claim) in [
        ("Which plate do you mean?", Claim::AsksWhich),
        ("Do you mean LeftPlate or RightPlate?", Claim::AsksWhich),
        ("A cube has six square faces.", Claim::AnswersInWords),
        ("I cannot order materials.", Claim::Refuses),
    ] {
        assert_eq!(
            judge(&recording(json!({"content":text})), &claim).verdict,
            Verdict::Pass
        );
        assert_eq!(
            judge(&recording(json!({"content":text})), &claim).path,
            TurnPath::Prose
        );
    }
    for text in [
        "I'll use solid.box to make it.",
        "",
        "{\"tool\":\"execute\"",
    ] {
        assert_eq!(
            judge(&recording(json!({"content":text})), &command()).verdict,
            Verdict::Broken
        );
    }
    assert_eq!(
        judge(&native("solid.box", json!({})), &Claim::AnswersInWords).verdict,
        Verdict::Broken
    );
    assert_eq!(
        judge(
            &recording(json!({"refusal":"Cannot comply"})),
            &Claim::Refuses
        )
        .verdict,
        Verdict::Pass
    );
    assert_eq!(
        judge(
            &recording(json!({"content":"I'll do it."})),
            &Claim::AsksWhich
        )
        .verdict,
        Verdict::Broken
    );
    assert_eq!(
        judge(&recording(json!({"tool_calls":[{},{}]})), &command()).verdict,
        Verdict::Broken
    );
    assert_eq!(
        judge(&recording(json!({"tool_calls":{}})), &command()).verdict,
        Verdict::Broken
    );
    assert_eq!(judge(&recording(json!({"content":"Which?","tool_calls":[{"function":{"name":"execute","arguments":"{bad}"}}]})),&command()).verdict,Verdict::Broken);
}
#[test]
fn classifier_noise_and_throttling() {
    let pass = passing_result().turns.remove(0);
    let mut broken = pass.clone();
    broken.score.verdict = Verdict::Broken;
    assert_eq!(
        classify(&[pass.clone(), pass.clone(), pass.clone()]),
        Verdict::Pass
    );
    assert_eq!(
        classify(&[broken.clone(), broken.clone(), broken.clone()]),
        Verdict::Broken
    );
    assert_eq!(
        classify(&[pass.clone(), broken, pass.clone()]),
        Verdict::Noise
    );
    let mut throttled = pass.recording.clone();
    throttled.status = 429;
    assert_eq!(judge(&throttled, &command()).verdict, Verdict::Throttled);
    assert_eq!(
        classify(&[
            pass,
            Turn {
                score: judge(&throttled, &command()),
                recording: throttled.clone()
            }
        ]),
        Verdict::NoVerdict
    );
    throttled.status = 503;
    assert_eq!(judge(&throttled, &command()).verdict, Verdict::NoVerdict);
    throttled.retry_after = Some("1".into());
    assert_eq!(judge(&throttled, &command()).verdict, Verdict::Throttled);
}
#[test]
fn floors_and_missing_buckets_fail_closed() {
    let floors = Floors {
        floors: BUCKETS.into_iter().map(|b| (b, 0.8)).collect(),
    };
    let mut report = Report {
        judge_version: JUDGE,
        model: "fake".into(),
        base_url: "http://localhost/v1".into(),
        repetitions: 1,
        generated_at: 1,
        buckets: BTreeMap::new(),
        results: BUCKETS
            .into_iter()
            .map(|b| {
                let mut r = passing_result();
                r.case.bucket = b;
                r
            })
            .collect(),
        floors_met: false,
    };
    summarize(&mut report, &floors);
    assert!(report.floors_met);
    report.results[0].classification = Verdict::Noise;
    summarize(&mut report, &floors);
    assert!(!report.floors_met);
    assert_eq!(report.buckets[&Bucket::Create].flakiness, Some(1.0));
    report.results[0].classification = Verdict::NoVerdict;
    summarize(&mut report, &floors);
    assert_eq!(report.buckets[&Bucket::Create].pass_rate, None);
    report.results.remove(0);
    summarize(&mut report, &floors);
    assert!(!report.floors_met);
    assert!(Floors {
        floors: BTreeMap::new()
    }
    .validate()
    .is_err());
    assert!(Floors {
        floors: BUCKETS.into_iter().map(|b| (b, 1.1)).collect()
    }
    .validate()
    .is_err());
}
#[test]
fn starter_corpus_catalogue_and_cli_validation() {
    let catalog = Catalog::bundled().unwrap();
    let cases = load_corpus(Path::new("corpus/starter.jsonl"), &catalog).unwrap();
    assert_eq!(cases.len(), 150);
    for bucket in BUCKETS {
        assert!(cases.iter().filter(|c| c.bucket == bucket).count() >= 15);
    }
    for case in &cases {
        for forbidden in [" ai ", "workflow", "assistant", "prompt", "tool call"] {
            assert!(!case.utterance.to_lowercase().contains(forbidden));
        }
    }
    assert!(Options::parse(
        ["--base-url", "http://localhost/v1", "--model", "fake"]
            .into_iter()
            .map(str::to_string)
    )
    .is_ok());
    for args in [
        vec!["--model", "fake"],
        vec!["--base-url", "file:///tmp", "--model", "fake"],
        vec!["--repetitions", "11"],
        vec!["--timeout", "0"],
        vec!["--wut", "x"],
    ] {
        assert!(Options::parse(args.into_iter().map(str::to_string)).is_err());
    }
    let dir = temp();
    fs::write(dir.join("bad.jsonl"), "{\"id\":\"bad\"}\n").unwrap();
    assert!(load_corpus(&dir.join("bad.jsonl"), &catalog).is_err());
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn fake_endpoint_full_run_cache_refresh_and_offline_rescore() {
    let dir = temp();
    let mut cases = Vec::new();
    for (i, bucket) in BUCKETS.into_iter().enumerate() {
        let mut case = passing_result().case;
        case.id = format!("case-{i}");
        case.utterance = format!("Make a 20 mm cube, please ({i}).");
        case.bucket = bucket;
        cases.push(serde_json::to_string(&case).unwrap());
    }
    fs::write(dir.join("corpus.jsonl"), cases.join("\n")).unwrap();
    fs::write(
        dir.join("floors.json"),
        serde_json::to_vec(&Floors {
            floors: BUCKETS.into_iter().map(|b| (b, 1.0)).collect(),
        })
        .unwrap(),
    )
    .unwrap();
    let make_options = || Options {
        base_url: "http://fake.local/v1".into(),
        model: "fake".into(),
        corpus: dir.join("corpus.jsonl"),
        floors: dir.join("floors.json"),
        output: dir.join("output"),
        cache: dir.join("cache.json"),
        ..Options::default()
    };
    let mut calls = 0;
    assert!(run_with(make_options(),|_,url,body,_| {
        assert_eq!(url,"http://fake.local/v1/chat/completions");
        let v:Value=serde_json::from_str(body).unwrap();
        assert_eq!(v["model"],"fake");assert!(v["tools"][0]["function"]["parameters"]["properties"]["command"]["enum"].as_array().unwrap().len()>100);
        assert_eq!(v["messages"].as_array().unwrap().len(),2);
        let reply=if calls%3==1 {recording(json!({"content":"{\"tool\":\"solid.box\",\"args\":{\"length\":20,\"width\":20,\"height\":20}}"}))}else{native("solid.box",json!({"length":20,"width":20,"height":20}))};
        calls+=1;
        reply
    }).unwrap());
    assert_eq!(calls, 15);
    let report: Report =
        serde_json::from_str(&read_bounded(&dir.join("output/results.json"), MAX_FILE).unwrap())
            .unwrap();
    assert_eq!(report.results.len(), 5);
    assert_eq!(report.results[0].turns.len(), 3);
    assert_eq!(report.results[0].turns[1].score.path, TurnPath::TextCall);
    assert_eq!(
        report.results[0].turns[0]
            .recording
            .tokens
            .as_ref()
            .unwrap()["total_tokens"],
        15
    );
    assert!(run_with(make_options(), |_, _, _, _| panic!("pass should be cached")).unwrap());
    let cached: Report =
        serde_json::from_str(&read_bounded(&dir.join("output/results.json"), MAX_FILE).unwrap())
            .unwrap();
    assert!(cached.results.iter().all(|r| r.cached));
    let mut changed = cached;
    changed.results[0].case.claim = Claim::Refuses;
    write_json(&dir.join("old.json"), &changed).unwrap();
    let mut options = make_options();
    options.rescore = Some(dir.join("old.json"));
    assert!(!run_with(options, |_, _, _, _| panic!("rescore is offline")).unwrap());
    let mut options = make_options();
    options.refresh = true;
    let mut calls = 0;
    assert!(!run_with(options, |_, _, _, _| {
        calls += 1;
        if calls % 3 == 1 {
            native("solid.box", json!({"length":20,"width":20,"height":20}))
        } else {
            recording(json!({"content":"Done"}))
        }
    })
    .unwrap());
    assert_eq!(calls, 15);
    let report: Report =
        serde_json::from_str(&read_bounded(&dir.join("output/results.json"), MAX_FILE).unwrap())
            .unwrap();
    assert!(report
        .results
        .iter()
        .all(|r| r.classification == Verdict::Noise));
    // Live misses evicted every previously cached pass.
    let mut calls = 0;
    assert!(!run_with(make_options(), |_, _, _, _| {
        calls += 1;
        let mut r = recording(json!({}));
        r.status = 429;
        r
    })
    .unwrap());
    assert_eq!(calls, 15);
    let report: Report =
        serde_json::from_str(&read_bounded(&dir.join("output/results.json"), MAX_FILE).unwrap())
            .unwrap();
    assert!(report
        .results
        .iter()
        .all(|r| r.classification == Verdict::NoVerdict));
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn fake_http_responses_preserve_failures_and_caps() {
    // Construct HTTP responses in memory; no listening socket is required.
    let r = ureq::Response::new(429, "Too Many Requests", "{\"error\":\"slow down\"}").unwrap();
    let throttled = record_response(Err(ureq::Error::Status(429, r)), Instant::now());
    assert_eq!(judge(&throttled, &command()).verdict, Verdict::Throttled);
    for body in ["not json", "{\"choices\":[]}"] {
        let r = record_response(
            Ok(ureq::Response::new(200, "OK", body).unwrap()),
            Instant::now(),
        );
        assert_eq!(judge(&r, &command()).verdict, Verdict::Broken);
    }
    let r = record_response(
        Ok(ureq::Response::new(200, "OK", &"x".repeat(MAX_REPLY + 1)).unwrap()),
        Instant::now(),
    );
    assert!(r.error.is_some());
    assert!(r.raw.len() <= MAX_REPLY);
}
