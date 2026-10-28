//! 日志门禁还没改完的那张单子（施工 LG-1 下清空）。

/// 还没改完的（施工 LG-1 下清空）：文件、格名，照为什么分组。照「写什么、不写什么」那张表都要改，LG-1 上先改要紧的（路径、
/// 会话里来的网址、外部身份、工具的结果、panic 的原话、会话日志的错误）。
pub const PENDING: &[(&str, &str)] = &[
    // 人起的名字：人格、预设、知识库、密钥（写成短编号或者不写）。
    ("crates/miyu-endpoint/src/personas.rs", "persona"),
    ("crates/miyu-endpoint/src/personas/picture.rs", "persona"),
    ("crates/miyu-endpoint/src/personas/write.rs", "persona"),
    ("crates/miyu-session/src/actor/persona.rs", "persona"),
    ("crates/miyu-session/src/open.rs", "persona"),
    ("crates/miyu-endpoint/src/presets/write.rs", "preset"),
    ("crates/miyu-endpoint/src/packages/port.rs", "preset"),
    ("crates/miyu-endpoint/src/knowledge.rs", "kb"),
    ("crates/miyu-session/src/knowledge.rs", "kb"),
    ("crates/miyu-endpoint/src/secrets.rs", "name"),
    ("crates/miyu-endpoint/src/mcp/secrets.rs", "secret"),
    // 数据根里的文件、目录（带账号的 `…`）改成 `layer=`、`kind=`。
    ("crates/miyu-core/src/lib.rs", "root"),
    ("crates/miyu-core/src/sandbox.rs", "helper"),
    ("crates/miyu-core/src/settings.rs", "file"),
    ("crates/miyu-endpoint/src/config.rs", "file"),
    ("crates/miyu-endpoint/src/config/forget.rs", "file"),
    ("crates/miyu-endpoint/src/config/journal.rs", "file"),
    ("crates/miyu-endpoint/src/config/observe.rs", "file"),
    ("crates/miyu-endpoint/src/config/rewrite.rs", "file"),
    ("crates/miyu-endpoint/src/config/set.rs", "file"),
    ("crates/miyu-endpoint/src/config/trusting.rs", "file"),
    ("crates/miyu-endpoint/src/extensions.rs", "file"),
    ("crates/miyu-endpoint/src/mcp/cache.rs", "dir"),
    ("crates/miyu-endpoint/src/mcp/cache.rs", "file"),
    ("crates/miyu-endpoint/src/packages.rs", "file"),
    ("crates/miyu-endpoint/src/provide/cache.rs", "file"),
    ("crates/miyu-endpoint/src/scripts.rs", "path"),
    ("crates/miyu-endpoint/src/secrets.rs", "file"),
    ("crates/miyu-ipc/src/lib.rs", "socket"),
    ("crates/miyu-ipc/src/unix.rs", "dir"),
    ("crates/miyu-ipc/src/unix.rs", "socket"),
    // 人写的字：配置写错的原文、读不懂的值、软件源的地址（只写主机）。
    ("crates/miyu-core/src/settings.rs", "value"),
    ("crates/miyu-endpoint/src/mcp/home.rs", "detail"),
    ("crates/miyu-endpoint/src/packages/source/sync.rs", "base"),
];
