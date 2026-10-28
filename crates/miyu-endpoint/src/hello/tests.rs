//! 身份和「替谁干活」分开以后的守卫（施工 Y-5 上）：`caller.account`（这个连接是谁）只在身份那几处用——凭据、吊销、握手、
//! 账号方法、成员闸和桥、上传和附件「谁交的」；资源归谁、会话属主一律 `caller.acting_for()`。别处新用了 `caller.account`，这里
//! 报红：Y-5 中让核心拉起的包是自己的账号以后，两个就不是同一个了。

use std::collections::BTreeMap;
use std::path::Path;

/// 用到 `caller.account` 的文件和次数（相对 `src/`）。
const IDENTITY: [(&str, usize); 7] = [
    // 包调包时答的那一份的 blob 拷给调的那条连接（施工 IM-1 中下）：它照 `blob.get` 读，`blob.get` 照谁交的找。
    ("backstage/calls.rs", 1),
    // 吊销、身份那一格、握手、握手认出来的、登录凭据、账号方法两处。
    ("connection.rs", 2),
    // `blob.put`、`blob.open`、`blob.get`：上传的东西归谁交的。
    ("methods.rs", 3),
    // 附件照谁交的找、交给会话属主（`attach::blocks`、`attach::hand_over`）。
    ("methods/session.rs", 7),
    // `model.call` 的图先照谁交的找（`blob.put` 存在那里，施工 BUG-2），找不到再照替谁干活的。
    ("methods/later.rs", 1),
    // 会话里的话、命令是谁说的（`sessions::person`，施工 Y-2 下下上）：记的是说话的人，身份。
    ("sessions.rs", 1),
    // 连接是不是平台桥（施工 IM-2）：照身份认系统账号，同 `connection.rs` 的桥。
    ("venues/bridge.rs", 2),
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
        "caller.account 只给身份那几处用（施工 Y-5 上）。多出来的那一处是身份（凭据、吊销、成员闸、桥、上传和附件谁交的）就把 \
         IDENTITY 里那个文件的数加上；是替谁干活（资源归谁、会话属主）就改读 caller.acting_for()"
    );
}
