//! `miyu dev driver-check` 印的样子（施工 MD-3）：一个驱动一行、一份样本一行，原因接在后面；有红的不算过，跳过的算过。

use serde_json::json;

use super::*;

#[test]
fn the_report_lists_drivers_and_samples_and_fails_on_red() {
    let result = json!({"package": "ag", "drivers": [
        {"driver": "ag/ag", "cache": "contract", "result": "failed", "samples": [
            {"sample": "plain", "ok": true},
            {"sample": "pair", "ok": false, "why": "adding a message at the end changed byte 9"}
        ]},
        {"driver": "ag/lite", "cache": "best_effort", "result": "skipped", "why": "driver.encode is not implemented"}
    ]});
    let (text, passed) = report(&result, Language::English);
    assert!(!passed);
    assert_eq!(
        text,
        "ag/ag (contract) failed\n  ok plain\n  failed pair: adding a message at the end changed byte 9\nag/lite (best_effort) skipped: driver.encode is not implemented"
    );
    let (text, _) = report(&result, Language::Chinese);
    assert!(
        text.starts_with("ag/ag（contract）没通过\n  通过 plain\n  没通过 pair: "),
        "{text}"
    );
    let skipped = json!({"drivers": [{"driver": "ag/lite", "cache": "none", "result": "skipped"}]});
    assert!(report(&skipped, Language::English).1, "只有跳过的算过");
}
