use dutchman_pilot::recipe_library::Library;
use serde_json::Value;
use std::io::Read;

fn report(text: &str, enforce: bool) -> Result<(), Box<dyn std::error::Error>> {
    let rows: Vec<Value> = serde_json::from_str(text)?;
    assert!(rows.len() <= 256);
    let lib = Library::embedded()?;
    let mut counts = [[0usize; 2]; 3];
    let (mut positives, mut negatives, mut false_fill, mut recovered) = (0, 0, 0, 0);
    for row in &rows {
        let text = row["text"].as_str().ok_or("missing text")?;
        let found = lib.form_band(text)?;
        let band = found["band"].as_str().ok_or("missing band")?;
        let positive = row["recipe"] != "none";
        let correct = if band == "fill" {
            found["recipes"][0]["id"] == row["recipe"]
        } else if band == "route" {
            found["recipes"]
                .as_array()
                .is_some_and(|rs| rs.iter().any(|r| r["id"] == row["recipe"]))
        } else {
            !positive
        };
        let index = match band {
            "fill" => 0,
            "route" => 1,
            _ => 2,
        };
        counts[index][0] += 1;
        counts[index][1] += usize::from(correct);
        if positive {
            positives += 1;
            recovered += usize::from(correct && band != "low");
            if !correct {
                eprintln!(
                    "MISS {} expected={} band={} candidates={:?}",
                    text,
                    row["recipe"],
                    band,
                    found["recipes"].as_array().map(|rs| rs
                        .iter()
                        .map(|r| (r["id"].clone(), r["score"].clone()))
                        .collect::<Vec<_>>())
                );
            }
        } else {
            negatives += 1;
            false_fill += usize::from(band == "fill");
            if band == "fill" {
                eprintln!("FALSE FILL {text}");
            }
        }
    }
    for (band, c) in ["fill", "route", "low"].iter().zip(counts) {
        eprintln!(
            "{band}: offered={} correct={} precision={:.2}% recall={:.2}%",
            c[0],
            c[1],
            100.0 * c[1] as f64 / c[0].max(1) as f64,
            100.0 * c[1] as f64 / if band == &"low" { negatives } else { positives }.max(1) as f64
        );
    }
    eprintln!("TOTAL n={} positive={positives} none={negatives} recovered={recovered} recall={:.2}% false_fill={false_fill} ({:.2}%)", rows.len(), 100.0*recovered as f64/positives as f64,100.0*false_fill as f64/negatives as f64);
    if enforce {
        assert!(recovered * 100 >= positives * 90);
        assert!(false_fill * 100 <= negatives * 5);
    }
    Ok(())
}

#[test]
fn tune_ranking_sample() -> Result<(), Box<dyn std::error::Error>> {
    report(include_str!("fixtures/dev-route-tune.json"), true)
}

#[test]
fn bands_and_dimension_tokens() -> Result<(), Box<dyn std::error::Error>> {
    let lib = Library::embedded()?;
    assert_eq!(lib.form_band("write a sonnet")?["band"], "low");
    assert_eq!(lib.form_band("hinge pivot two parts")?["band"], "route");
    assert_eq!(
        lib.form_band("Extrude an L profile on XZ 47 wide 33 tall legs 5 thick depth 22")?["band"],
        "fill"
    );
    assert_eq!(
        lib.find(
            "a cylinder 10mm 30cm",
            &dutchman_pilot::recipe_library::Bm25
        )?,
        lib.find(
            "a cylinder 40mm 60cm",
            &dutchman_pilot::recipe_library::Bm25
        )?
    );
    Ok(())
}

#[test]
#[ignore = "one final external development check, never used for tuning"]
fn final_development_check() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::var("PILOT_ROUTE_CHECK")?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(262145)
        .read_to_end(&mut bytes)?;
    assert!(bytes.len() <= 262144);
    report(std::str::from_utf8(&bytes)?, false)
}
