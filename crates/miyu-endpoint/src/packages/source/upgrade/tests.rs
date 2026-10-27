//! 升级的盘点（施工 G-5）：从文件夹装的不碰；版本一样的不动；不一样的升（旧版也算）；`hold` 的跳过；来源变了、没有了、
//! 被拉黑的各报一句。

use serde_json::json;

use super::*;

fn installed(id: &str, version: Option<&str>, origin: Option<&str>) -> Installed {
    Installed {
        id: id.to_string(),
        version: version.map(str::to_string),
        origin: origin.map(str::to_string),
        feed: None,
    }
}

fn index() -> Index {
    let entry = |id: &str, version: &str, origin: &str| {
        json!({"id": id, "version": version, "origin": origin,
               "files": [{"url": "https://example.invalid/x", "sha256": "ab", "size": 1}]})
    };
    serde_json::from_value(json!({
        "version": 1,
        "generated": "2026-10-19T03:00:00Z",
        "packages": [
            entry("same", "1.0", "github.com/a/same"),
            entry("newer", "2.0", "github.com/a/newer"),
            entry("older", "0.9", "github.com/a/older"),
            entry("held", "2.0", "github.com/a/held"),
            entry("moved", "2.0", "github.com/b/moved"),
            entry("local", "9.0", "github.com/a/local"),
        ],
        "blocked": [{"id": "evil", "reason": "malware"}],
    }))
    .expect("读得成")
}

#[test]
fn the_survey_sorts_each_installed_package() {
    let all = [
        installed("same", Some("1.0"), Some("github.com/a/same")),
        installed("newer", Some("1.0"), Some("github.com/a/newer")),
        installed("older", Some("1.0"), Some("github.com/a/older")),
        installed("held", Some("1.0"), Some("github.com/a/held")),
        installed("moved", Some("1.0"), Some("github.com/a/moved")),
        installed("gone", Some("1.0"), Some("github.com/a/gone")),
        installed("evil", Some("1.0"), Some("github.com/a/evil")),
        installed("local", Some("1.0"), None),
    ];
    let indexes = Indexes::from([(OFFICIAL.to_string(), index())]);
    let survey = survey(&all, &indexes, &["held".to_string()]);
    assert_eq!(
        survey.upgrades,
        [
            (
                "newer".to_string(),
                Some("1.0".to_string()),
                "2.0".to_string()
            ),
            (
                "older".to_string(),
                Some("1.0".to_string()),
                "0.9".to_string()
            ),
        ],
        "不比大小：旧版也跟着回去"
    );
    assert_eq!(survey.held, ["held"]);
    assert_eq!(
        survey.moved,
        [(
            "moved".to_string(),
            "github.com/a/moved".to_string(),
            "github.com/b/moved".to_string()
        )]
    );
    assert_eq!(survey.gone, ["gone"]);
    assert_eq!(
        survey.blocked,
        [("evil".to_string(), "malware".to_string())]
    );
    let shown = shown(&survey);
    assert_eq!(
        shown["upgrades"][0],
        json!({"package": "newer", "from": "1.0", "to": "2.0"})
    );
}
