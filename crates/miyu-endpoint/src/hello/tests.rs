//! 身份和「替谁干活」分开以后的守卫（施工 Y-5 上）：`caller.account`（这个连接是谁）只在身份那几处用——凭据、吊销、握手、
//! 账号方法、成员闸和桥、上传和附件「谁交的」；资源归谁、会话属主一律 `caller.acting_for()`。别处新用了 `caller.account`，这里
//! 报红：Y-5 中让核心拉起的包是自己的账号以后，两个就不是同一个了。

use std::collections::BTreeMap;
use std::path::Path;

/// 用到 `caller.account` 的文件和次数（相对 `src/`）。
const IDENTITY: [(&str, usize); 3] = [
    // 吊销、身份那一格、握手、握手认出来的、登录凭据、账号方法两处。
    ("connection.rs", 7),
    // `blob.put`、`blob.open`、`blob.get`：上传的东西归谁交的。
    ("methods.rs", 3),
    // 附件照谁交的找、交给会话属主（`attach::blocks`、`attach::hand_over`）。
    ("methods/session.rs", 7),
];

/// `dir` 下每个 `.rs`（测试文件不算）里 `caller.account` 出现几次。
fn uses(src: &Path, dir: &Path, found: &mut BTreeMap<String, usize>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("读得了")
        .map(|entry| entry.expect("读得了").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            uses(src, &path, found);
            continue;
        }
        let tests = path.file_name().is_some_and(|name| name == "tests.rs")
            || path.components().any(|part| part.as_os_str() == "tests");
        if tests || path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("读得了");
        let count = text.matches("caller.account").count();
        if count > 0 {
            let relative = path.strip_prefix(src).expect("在 src 下面");
            let shown = relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            found.insert(shown, count);
        }
    }
}

#[test]
fn caller_account_is_only_used_for_identity() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = BTreeMap::new();
    uses(&src, &src, &mut found);
    let wanted: BTreeMap<String, usize> = IDENTITY
        .iter()
        .map(|(file, count)| ((*file).to_string(), *count))
        .collect();
    assert_eq!(
        found, wanted,
        "caller.account 只给身份那几处用；资源归谁、会话属主用 caller.acting_for()（施工 Y-5 上）"
    );
}
