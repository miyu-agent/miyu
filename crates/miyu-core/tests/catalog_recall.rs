//! 目录不常驻，核心起来时（施工 V-25 下，`docs/blueprint/models.md`「怎么走」第二条第 2 条）：头一回读整份目录、把配置里写过
//! 的记下写进 `state/models/catalog.json`；下一回版本对上了不读整份目录（把原文换成同样大小、同样改动时间的乱码，整理好的
//! 那份删掉：真读了就读不出来，答的却照旧）；记下的那份坏了、写法的版本不对、认原厂的表换了的，当没有、整份重算；后台拉到
//! 新目录，记下的那份照新目录换版本、写盘。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use serde_json::json;

use miyu_config::Layer;
use miyu_config::merge::{Layers, Resolved, merge};
use miyu_config::parse::parse;
use miyu_core::models::catalog::{FILE, META, Meta};
use miyu_core::models::digest;
use miyu_core::models::recall::Version;
use miyu_core::models::refresh::{Refreshed, Refresher};
use miyu_core::models::{begin, read_early};
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_http::{Proxy, fetcher};
use miyu_models::catalog::Recalled;
use miyu_models::facts::facts;
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_models::provider::provider;
use miyu_models::settings::{ModelSettings, PoolSettings, ProviderSettings, UseSettings};
use miyu_session::{ModelData, RECALL_FILE};

/// 用完就删的临时目录：快照（带认原厂的表）、缓存、`state/models`、一个当程序的文件。
struct Dir(PathBuf);

impl Dir {
    fn new() -> Dir {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let dir =
            std::env::temp_dir().join(format!("miyu-recall-{}-{stamp}-{n}", std::process::id()));
        for sub in ["snapshot", "cache", "state"] {
            std::fs::create_dir_all(dir.join(sub)).expect("建得了");
        }
        std::fs::write(dir.join("program"), "a program").expect("写得进");
        std::fs::write(
            dir.join("snapshot").join("vendors.toml"),
            "deepseek = [\"deepseek\"]\n",
        )
        .expect("写得进");
        let meta = Meta {
            source: "https://models.dev/api.json".to_string(),
            fetched: "2026-10-01T00:00:00.000Z".to_string(),
            etag: None,
        };
        std::fs::write(dir.join("snapshot").join(META), meta.to_json()).expect("写得进");
        std::fs::write(dir.join("snapshot").join(FILE), catalog(1000)).expect("写得进");
        Dir(dir)
    }

    fn at(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Dir {
    #[expect(clippy::let_underscore_must_use, reason = "删不掉就留在临时目录里")]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 只有一家 `p`（带地址）、一个模型 `m` 的目录，窗口是 `window`。
fn catalog(window: u64) -> String {
    json!({"p": {"id": "p", "api": "https://p.invalid/v1", "models": {"m": {"limit": {"context": window}}}}})
        .to_string()
}

const CONFIG: &str = "[providers.p]\nlocal = false\n\n[models]\nchat = \"p/m\"\n";

fn resolved() -> Resolved {
    let items = [
        ProviderSettings::ITEMS,
        ModelSettings::ITEMS,
        UseSettings::ITEMS,
        PoolSettings::ITEMS,
        miyu_models::settings::FIXED_POOL_ITEMS,
    ]
    .concat();
    let parsed = parse(&items, Layer::System, CONFIG).expect("写法对");
    assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
    let layers = Layers {
        system: Some(&parsed),
        ..Layers::default()
    };
    merge(&items, &layers, &|_| None)
}

/// 照 `dir` 起一回：读目录、读记下的那份，等读完。
async fn start(dir: &Dir) -> Arc<ModelData> {
    let data = Arc::new(ModelData::new(
        Profiles::default(),
        Vendors::default(),
        Some(dir.at("state")),
    ));
    let early = read_early(
        dir.at("snapshot"),
        Some(dir.at("cache")),
        Some(dir.at("program")),
        Some(dir.at("state")),
    );
    drop(begin(
        Arc::clone(&data),
        early,
        Some(dir.at("state")),
        Some(resolved()),
    ));
    tokio::time::timeout(Duration::from_secs(10), data.wait())
        .await
        .expect("读完了");
    data
}

/// `p/m` 的窗口：照记下的、或者整份目录。
fn window(data: &ModelData) -> Option<u64> {
    let resolved = resolved();
    let values = resolved.values();
    data.with(|knowledge| {
        let found = provider(&values, knowledge, "p").ok()?;
        facts(&resolved, knowledge, &found, "m").0.window.value
    })
}

fn recalled(dir: &Dir) -> Recalled {
    let text = std::fs::read_to_string(dir.at("state").join(RECALL_FILE)).expect("写了");
    Recalled::from_json(&text).expect("读得回来")
}

/// 等整理好的那份写出来（写的是另一个线程），再删掉。
async fn drop_digest(dir: &Dir) {
    let digest = dir.at("cache").join(digest::FILE);
    let written = tokio::time::timeout(Duration::from_secs(10), async {
        while !digest.is_file() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(written.is_ok(), "整理好的那份写出来了");
    tokio::time::sleep(Duration::from_millis(100)).await;
    std::fs::remove_file(&digest).expect("删得掉");
}

/// 把原文换成同样大小的乱码，改动时间照旧：指纹不变，真读就读不出来。
fn scramble(file: &Path) {
    let modified = std::fs::metadata(file)
        .and_then(|meta| meta.modified())
        .expect("有改动时间");
    let len = std::fs::metadata(file).expect("在").len();
    let garbage = "x".repeat(usize::try_from(len).expect("不大"));
    std::fs::write(file, garbage).expect("写得进");
    std::fs::File::options()
        .write(true)
        .open(file)
        .and_then(|opened| opened.set_modified(modified))
        .expect("改得回改动时间");
}

#[tokio::test]
async fn a_recall_that_matches_skips_reading_the_catalog() {
    let dir = Dir::new();
    let first = start(&dir).await;
    assert_eq!(window(&first), Some(1000));
    let saved = recalled(&dir);
    assert_eq!(saved.len(), (1, 1), "配置里写过的一家、一个模型");
    drop(first);
    drop_digest(&dir).await;
    scramble(&dir.at("snapshot").join(FILE));
    let second = start(&dir).await;
    assert_eq!(window(&second), Some(1000), "照记下的答，没读整份目录");
    assert!(second.catalog().is_none(), "真读整份目录是读不出来的");
    assert_eq!(recalled(&dir).of(), saved.of());
}

#[tokio::test]
async fn a_broken_or_old_recall_is_rebuilt_from_the_catalog() {
    let dir = Dir::new();
    drop(start(&dir).await);
    let good = recalled(&dir);
    let file = dir.at("state").join(RECALL_FILE);
    let text = good.to_json().expect("写得成");
    let older = text.replacen("{\"v\":1,", "{\"v\":0,", 1);
    assert_ne!(older, text);
    for broken in ["{", "not json at all", older.as_str()] {
        std::fs::write(&file, broken).expect("写得进");
        let data = start(&dir).await;
        assert_eq!(window(&data), Some(1000), "当没有，整份重算");
        assert_eq!(recalled(&dir), good, "重算完写回了好的一份");
    }
    // 认原厂的表换了：版本串对不上，整份重算。
    std::fs::write(
        dir.at("snapshot").join("vendors.toml"),
        "claude = [\"anthropic\"]\n",
    )
    .expect("写得进");
    let data = start(&dir).await;
    assert_eq!(window(&data), Some(1000));
    assert_ne!(recalled(&dir).of(), good.of(), "版本串跟着认原厂的表换");
    // 目录换了一份（同样的 `fetched`，原文变了）：也对不上，照新的记。
    std::fs::write(dir.at("snapshot").join(FILE), catalog(2000)).expect("写得进");
    let data = start(&dir).await;
    assert_eq!(window(&data), Some(2000));
}

#[tokio::test]
async fn a_refresh_moves_the_recall_to_the_new_catalog() {
    let dir = Dir::new();
    let data = start(&dir).await;
    let before = recalled(&dir);
    let server = Server::start(vec![Reply {
        status: 200,
        headers: vec![("ETag".to_string(), "\"v1\"".to_string())],
        body: vec![Piece::Bytes(catalog(3000).into_bytes())],
    }])
    .await;
    let refresher = Refresher {
        data: Arc::clone(&data),
        client: fetcher(Proxy::Off).expect("造得出客户端"),
        cache: dir.at("cache"),
        version: Version::of_places(&dir.at("snapshot"), Some(&dir.at("program"))),
    };
    let url = format!("{}/api.json", server.base_url);
    assert_eq!(refresher.once(&url).await, Refreshed::Replaced);
    assert_eq!(window(&data), Some(3000), "记下的照新目录重算了");
    let after = recalled(&dir);
    assert_ne!(after.of(), before.of());
    assert_eq!(after.source().as_str(), "cache");
    // 下一回起来：新目录排第一，版本对上，不读整份目录。
    drop(data);
    let again = start(&dir).await;
    assert_eq!(window(&again), Some(3000));
    assert_eq!(recalled(&dir).of(), after.of());
}
