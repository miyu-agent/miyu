//! 出厂的 `search.json` 读得进、数对得上；写错的读不了。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::{Kind, load};

fn shipped() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

fn text() -> String {
    std::fs::read_to_string(shipped().join("software/net/search.json")).unwrap()
}

/// 一个临时的资源目录，`search.json` 写成 `text`；用完删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn with(text: &str) -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-net-search-{}-{n}", std::process::id()));
        let file = dir.join("software/net/search.json");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_shipped_rules_carry_the_order_of_the_design() {
    let rules = load(&shipped()).unwrap();
    // 设计 36 第二节第 2 条（没 key 的这几家；DeepSeek、网页抓取在 N-3）
    assert_eq!(
        rules.order,
        ["exa", "parallel", "firecrawl", "anysearch", "you"]
    );
    assert_eq!(
        rules.cooldown,
        [30, 60, 120, 360].map(|minutes| Duration::from_secs(minutes * 60)),
        "第 4 条：30 分钟、1 小时、2 小时，最多 6 小时"
    );
    assert_eq!(rules.sources["exa"].kind, Kind::Exa);
    assert_eq!(rules.sources["you"].name, "You.com");
    assert!(
        rules
            .sources
            .values()
            .all(|source| source.url.starts_with("https://"))
    );
}

#[test]
fn a_wrong_file_is_refused() {
    for (from, to) in [
        ("\"results\": 8", "\"results\": 0"),
        (
            "\"cooldown_minutes\": [30, 60, 120, 360]",
            "\"cooldown_minutes\": []",
        ),
        (
            "\"cooldown_minutes\": [30, 60, 120, 360]",
            "\"cooldown_minutes\": [30, 0]",
        ),
        ("\"order\": [\"exa\",", "\"order\": [\"nowhere\", \"exa\","),
        ("\"kind\": \"exa\"", "\"kind\": \"bing\""),
        ("\"results\": 8", "\"results\": 8, \"note\": 1"),
    ] {
        let changed = text().replacen(from, to, 1);
        assert_ne!(changed, text(), "{from} 在出厂的那一份里");
        assert!(load(&Scratch::with(&changed).0).is_err(), "{to}");
    }
}
