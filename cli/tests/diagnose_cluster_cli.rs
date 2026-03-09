use assert_cmd::cargo::cargo_bin_cmd;
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn diagnose_cluster_text_matches_golden() {
    let context_path = fixture_path("cluster_context.json");
    let expected = std::fs::read_to_string(fixture_path("cluster_report.golden.txt"))
        .expect("golden fixture should exist");

    let output = cargo_bin_cmd!("kdocter")
        .args([
            "diagnose",
            "cluster",
            "--context-file",
            context_path.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let actual = String::from_utf8(output).expect("stdout should be utf8");
    assert_eq!(actual, expected);
}

#[test]
fn diagnose_cluster_json_matches_golden() {
    let context_path = fixture_path("cluster_context.json");
    let expected = std::fs::read_to_string(fixture_path("cluster_report.golden.json"))
        .expect("json golden fixture should exist");

    let output = cargo_bin_cmd!("kdocter")
        .args([
            "diagnose",
            "cluster",
            "--output",
            "json",
            "--context-file",
            context_path.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let actual = String::from_utf8(output).expect("stdout should be utf8");
    assert_eq!(actual, expected);
}

#[test]
fn diagnose_cluster_sarif_matches_golden() {
    let context_path = fixture_path("cluster_context.json");
    let expected = std::fs::read_to_string(fixture_path("cluster_report.golden.sarif.json"))
        .expect("sarif golden fixture should exist");

    let output = cargo_bin_cmd!("kdocter")
        .args([
            "diagnose",
            "cluster",
            "--output",
            "sarif",
            "--context-file",
            context_path.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let actual = String::from_utf8(output).expect("stdout should be utf8");
    assert_eq!(actual, expected);
}
