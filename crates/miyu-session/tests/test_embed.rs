//! 测试用的小程序 `miyu-test-embed`（施工 S-7 上）自己：照手造小模型的手算和 ONNX Runtime 算的（`expected.json`）每格差不过
//! 1e-6，超长的截掉、词中间的段带 `##` 查词表；读不懂的回错、接着读。

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::support::package::{program, tiny};

/// 照 `expected.json` 的六组编号写的句子，先后一样。
const SAID: [&str; 6] = ["我的猫", "喝茶", "hello world!", "x", "猫猫猫猫猫猫", ""];

#[test]
fn the_stand_in_matches_onnx_runtime_on_the_tiny_model() {
    let mut child = Command::new(program())
        .arg("--manifest")
        .arg(tiny().join("manifest.toml"))
        .arg("--dir")
        .arg(tiny())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("拉得起");
    let mut input = child.stdin.take().expect("有标准输入");
    for (at, text) in SAID.iter().chain(["unaffable"].iter()).enumerate() {
        writeln!(
            input,
            "{}",
            serde_json::json!({"id": at.to_string(), "text": text})
        )
        .expect("写得进");
    }
    writeln!(input, "not json").expect("写得进");
    drop(input);
    let out = child.wait_with_output().expect("跑完了");
    assert!(out.status.success(), "{out:?}");
    let lines: Vec<Value> = String::from_utf8(out.stdout)
        .expect("UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("一行一个 JSON"))
        .collect();
    assert_eq!(lines[0]["ready"]["model"], "local:tiny");
    let vector = |line: &Value| -> Vec<f64> {
        line["vector"]
            .as_array()
            .expect("有向量")
            .iter()
            .map(|x| x.as_f64().expect("是数"))
            .collect()
    };
    let expected: Value = serde_json::from_str(
        &std::fs::read_to_string(tiny().join("expected.json")).expect("读得到"),
    )
    .expect("是 JSON");
    for (at, want) in expected.as_array().expect("一串").iter().enumerate() {
        let got = vector(&lines[at + 1]);
        let want = vector(want);
        assert!(
            got.iter().zip(&want).all(|(a, b)| (a - b).abs() <= 1e-6),
            "{}：{got:?} 对 {want:?}",
            SAID[at]
        );
    }
    assert_ne!(
        vector(&lines[7]),
        vector(&lines[4]),
        "unaffable 切成 un ##aff ##able，不是 [UNK]"
    );
    assert!(lines[8]["error"].is_string(), "读不懂的回错：{}", lines[8]);
}
