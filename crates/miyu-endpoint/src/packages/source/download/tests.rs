//! 下包文件（施工 AS-2）：文件名拼出来不在缓存目录正下面的，不下、不读、不写。

use std::fs;
use std::path::PathBuf;

use miyu_source::index::File;

use super::{Wanted, fetch, hex};

/// 一个用完就删的临时目录（真实的位置）。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        let dir = std::env::temp_dir().join(format!("miyu-download-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Scratch(fs::canonicalize(dir).unwrap())
    }
}

impl Drop for Scratch {
    #[expect(clippy::let_underscore_must_use, reason = "删不掉就留在临时目录里")]
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 缓存外面放着一份哈希对得上的文件：名字写成 `../<它>` 的不认它、也不去下（原来照「缓存里已经有」交回缓存外面的路径，
/// 再装的就是它；能下的话 `.part` 换过去时写到缓存外面）。
#[tokio::test]
async fn a_name_that_leaves_the_cache_is_refused() {
    let scratch = Scratch::new();
    let cache = scratch.0.join("cache");
    fs::create_dir_all(&cache).unwrap();
    let body = b"package bytes";
    fs::write(scratch.0.join("escape.miyu"), body).unwrap();
    let file = File {
        target: None,
        url: "https://unreachable.invalid/escape.miyu".to_string(),
        sha256: hex(&<sha2::Sha256 as sha2::Digest>::digest(body)),
        size: body.len() as u64,
    };
    for name in ["../escape.miyu", "..", "a/b.miyu", ""] {
        let wanted = Wanted {
            name,
            file: &file,
            mirrors: &[],
        };
        let got = fetch(&cache, &wanted, &|_, _| {}).await;
        assert!(
            got.as_ref()
                .is_err_and(|why| why.contains("not a plain file name")),
            "{name:?}：{got:?}"
        );
    }
    assert_eq!(fs::read_dir(&cache).unwrap().count(), 0, "缓存里什么都没写");
}
