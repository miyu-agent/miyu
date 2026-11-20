//! 升级的盘点（施工 G-5）：从文件夹装的不碰；版本一样的不动；不一样的升（旧版也算）；`hold` 的跳过；来源变了、没有了、
//! 被拉黑的各报一句。系统加装的、出厂的不碰，新版要更新的 Miyu 的不算能升（施工 FX-7）。

use serde_json::json;

use super::*;

fn installed(id: &str, version: Option<&str>, origin: Option<&str>) -> Installed {
    Installed {
        id: id.to_string(),
        layer: Layer::Home,
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
    let survey = survey(&all, &indexes, &["held".to_string()], "2026.11.0");
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

/// 项目主人本机 2026-11-20 撞上的（施工 FX-7）：pacman 装的终端、网页认的那一份在系统加装这一层，本地库里却留着同编号、记了
/// 来源仓库的旧记录，盘点把它们当成能升的；新版要 Miyu 2026.11.0，正在跑的是候选版，装的时候整次失败。系统加装的、出厂的不看；
/// 索引写了更新的 Miyu 的进 `needs_newer_miyu`、别的照升；写的 Miyu 用得了的、没写的照升，开发版都用得了。
#[test]
fn distro_packages_are_left_alone_and_a_newer_miyu_is_skipped_not_failed() {
    let file = json!([{"url": "https://example.invalid/x", "sha256": "ab", "size": 1}]);
    let index: Index = serde_json::from_value(json!({
        "version": 1,
        "generated": "2026-11-20T03:00:00Z",
        "packages": [
            {"id": "tui", "version": "2026.11.0", "origin": "github.com/a/tui", "miyu": "2026.11.0", "files": file},
            {"id": "webui", "version": "2026.11.0", "origin": "github.com/a/webui", "miyu": "2026.11.0", "files": file},
            {"id": "im", "version": "0.3.2", "origin": "github.com/a/im", "miyu": "2026.11.0", "files": file},
            {"id": "memes", "version": "1.1", "origin": "github.com/a/memes", "miyu": "2026.11.0-rc.2.4", "files": file},
            {"id": "weather", "version": "2.0", "origin": "github.com/a/weather", "files": file},
        ],
    }))
    .expect("读得成");
    let at = |id: &str, layer: Layer| Installed {
        layer,
        ..installed(
            id,
            Some("2026.11.0-rc.2.4"),
            Some(&format!("github.com/a/{id}")),
        )
    };
    let all = [
        at("tui", Layer::Distro),
        at("webui", Layer::Shipped),
        at("im", Layer::Home),
        at("memes", Layer::Home),
        at("weather", Layer::Home),
    ];
    let indexes = Indexes::from([(OFFICIAL.to_string(), index)]);
    let surveyed = survey(&all, &indexes, &[], "2026.11.0-rc.2.4");
    let ids = |rows: &[(String, Option<String>, String)]| -> Vec<String> {
        rows.iter().map(|(id, ..)| id.clone()).collect()
    };
    assert_eq!(ids(&surveyed.upgrades), ["memes", "weather"]);
    assert_eq!(
        surveyed.needs_newer_miyu,
        [(
            "im".to_string(),
            "0.3.2".to_string(),
            "2026.11.0".to_string()
        )]
    );
    assert!(surveyed.gone.is_empty() && surveyed.moved.is_empty());
    assert_eq!(
        shown(&surveyed)["needs_newer_miyu"],
        json!([{"package": "im", "to": "0.3.2", "miyu": "2026.11.0"}])
    );
    // 正式版的核心用得了；开发版都用得了。
    for running in ["2026.11.0", "0.0.0"] {
        let surveyed = survey(&all, &indexes, &[], running);
        assert_eq!(
            ids(&surveyed.upgrades),
            ["im", "memes", "weather"],
            "{running}"
        );
        assert!(surveyed.needs_newer_miyu.is_empty(), "{running}");
    }
}
