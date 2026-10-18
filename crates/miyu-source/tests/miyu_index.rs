//! 真跑 `miyu-index`（施工 G-2 上）：建钥匙、签、验；私钥文件不覆盖、只给自己读写；读不成的索引不签；改过的索引验不过；没有
//! 私钥、参数不对的退出码 1、2。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const INDEX: &str = "{\"version\": 1, \"generated\": \"2026-10-19T03:00:00Z\", \"packages\": []}\n";

/// 测试用的临时目录：照真实位置给，用完删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("miyu-index-{name}-{}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("删得掉");
        }
        std::fs::create_dir_all(&dir).expect("建得了");
        Scratch(std::fs::canonicalize(&dir).expect("在"))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!("临时目录没删掉：{error}");
        }
    }
}

fn run(args: &[&str], key: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_miyu-index"));
    command.args(args).env_remove("MIYU_INDEX_KEY");
    if let Some(key) = key {
        command.env("MIYU_INDEX_KEY", key);
    }
    command.output().expect("跑得起来")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn path(path: &Path) -> &str {
    path.to_str().expect("是 UTF-8")
}

#[test]
fn keys_sign_and_verify_an_index() {
    let scratch = Scratch::new("sign");
    let secret = scratch.0.join("secret.key");
    let made = run(&["keygen", path(&secret)], None);
    assert_eq!(made.status.code(), Some(0), "{made:?}");
    let out = text(&made.stdout);
    let id = out
        .lines()
        .find_map(|line| line.strip_prefix("key "))
        .expect("印了编号")
        .to_string();
    let public = out
        .lines()
        .find_map(|line| line.strip_prefix("public "))
        .expect("印了公钥")
        .to_string();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&secret).expect("在").permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "私钥只给自己读写");
    }
    let again = run(&["keygen", path(&secret)], None);
    assert_eq!(again.status.code(), Some(1), "已经有的不覆盖");

    let index = scratch.0.join("index.json");
    std::fs::write(&index, INDEX).expect("写得进");
    let key = std::fs::read_to_string(&secret).expect("读得了");
    let signed = run(&["sign", path(&index)], Some(&key));
    assert_eq!(signed.status.code(), Some(0), "{signed:?}");
    assert!(scratch.0.join("index.json.sig").is_file());
    let good = run(&["verify", path(&index), "--public", &public], None);
    assert_eq!(good.status.code(), Some(0), "{good:?}");
    assert_eq!(
        text(&good.stdout),
        format!("good signature from key {id}\n")
    );

    let by_file = run(&["sign", path(&index), "--key", path(&secret)], None);
    assert_eq!(
        by_file.status.code(),
        Some(0),
        "照 --key 的文件签：{by_file:?}"
    );
    let empty_env = run(&["sign", path(&index), "--key", path(&secret)], Some(""));
    assert_eq!(
        empty_env.status.code(),
        Some(0),
        "环境变量是空的照 --key 签：{empty_env:?}"
    );

    std::fs::write(&index, INDEX.replace("03:00", "04:00")).expect("写得进");
    let changed = run(&["verify", path(&index), "--public", &public], None);
    assert_eq!(changed.status.code(), Some(1), "改过的验不过");
    assert!(
        text(&changed.stderr).contains("does not match"),
        "{changed:?}"
    );
    let official = run(&["verify", path(&index)], None);
    assert_eq!(official.status.code(), Some(1), "官方的钥匙不认这一把");
}

#[test]
fn bad_inputs_are_refused() {
    let scratch = Scratch::new("bad");
    let index = scratch.0.join("index.json");
    std::fs::write(&index, "{\"version\": 2}").expect("写得进");
    let secret = scratch.0.join("secret.key");
    assert_eq!(run(&["keygen", path(&secret)], None).status.code(), Some(0));
    let key = std::fs::read_to_string(&secret).expect("读得了");
    let unreadable = run(&["sign", path(&index)], Some(&key));
    assert_eq!(unreadable.status.code(), Some(1), "读不成的不签");
    assert!(!scratch.0.join("index.json.sig").exists());
    std::fs::write(&index, INDEX).expect("写得进");
    let keyless = run(&["sign", path(&index)], None);
    assert_eq!(keyless.status.code(), Some(1), "没有私钥");
    assert!(text(&keyless.stderr).contains("MIYU_INDEX_KEY"));
    for args in [
        &[][..],
        &["sign"][..],
        &["verify", "x", "--publik", "y"][..],
        &["what"][..],
    ] {
        assert_eq!(run(args, None).status.code(), Some(2), "{args:?}");
    }
}
