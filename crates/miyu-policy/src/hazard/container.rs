//! 容器把宿主交出去（施工 5-17 三补，2026-10-22 项目主人定）：`docker`、`podman`、`nerdctl` 的 `run`、`create`。
//!
//! - 整个交出去的拒（[`Kind::ContainerHost`]）：`--privileged`；`--cap-add` 加 `ALL`、`SYS_ADMIN`；把宿主的 `/`、`/dev`、
//!   `/sys` 整个挂进去（`-v`、`--volume`、`--mount` 的 `source`）。那天一个特权容器里跑 systemd，容器的 udev 把整机设备重新
//!   触发一遍，桌面、终端都坏了，只能按电源键（`--cgroupns=private` 挡不住）。
//! - 交出一部分的问人（[`Kind::ContainerShare`]）：`--device`、`--device-cgroup-rule`；`--pid`、`--net`、`--network`、`--ipc` 写 `host`；挂宿主
//!   `/dev`、`/sys` 下面的某一个。
//!
//! 入口是 systemd、`/sbin/init` 本身不拦：不带特权的 `podman --systemd=always` 正是测 systemd 单元该用的办法（扩展线 10-22
//! 建议拦，主会话没收：拦它会挡掉安全的那条路）。
//!
//! 只照字面认，各个选项的写法照 `--选项=值` 和 `--选项 值` 两种都认；镜像后面的参数也一起扫，误报的只是多问一次。

use super::Kind;
use super::lex::Word;

/// 认这些程序。
const ENGINES: [&str; 3] = ["docker", "podman", "nerdctl"];

/// `rest` 是程序名后面的词：是 `run`、`create`（`container run` 也算）的，认出来的交回。
pub(super) fn container(name: &str, rest: &[Word]) -> Option<Kind> {
    if !ENGINES.contains(&name) {
        return None;
    }
    let words: Vec<&str> = rest.iter().map(|word| word.text.as_str()).collect();
    let sub = words.iter().position(|word| !word.starts_with('-'))?;
    let at = match words[sub] {
        "run" | "create" => sub + 1,
        "container" if matches!(words.get(sub + 1), Some(&"run" | &"create")) => sub + 2,
        _ => return None,
    };
    let mut found = None;
    let mut index = at;
    while let Some(word) = words.get(index) {
        let (flag, inline) = match word.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, Some(value)),
            _ => (*word, None),
        };
        let takes = matches!(
            flag,
            "-v" | "--volume"
                | "--mount"
                | "--cap-add"
                | "--device"
                | "--device-cgroup-rule"
                | "--pid"
                | "--net"
                | "--network"
                | "--ipc"
        );
        let value = match (inline, takes) {
            (Some(value), _) => Some(value),
            (None, true) => {
                index += 1;
                words.get(index).copied()
            }
            (None, false) => None,
        };
        let kind = judge(flag, value.unwrap_or_default());
        if kind == Some(Kind::ContainerHost) {
            return kind;
        }
        found = found.or(kind);
        index += 1;
    }
    found
}

/// 一个选项和它的值。
fn judge(flag: &str, value: &str) -> Option<Kind> {
    match flag {
        "--privileged" => Some(Kind::ContainerHost),
        "--cap-add" => {
            let cap = value.to_ascii_uppercase();
            let cap = cap.strip_prefix("CAP_").unwrap_or(&cap);
            matches!(cap, "ALL" | "SYS_ADMIN").then_some(Kind::ContainerHost)
        }
        "-v" | "--volume" => mounted(value.split(':').next().unwrap_or_default()),
        "--mount" => value
            .split(',')
            .find_map(|part| {
                part.strip_prefix("source=")
                    .or_else(|| part.strip_prefix("src="))
            })
            .and_then(mounted),
        "--device" | "--device-cgroup-rule" => Some(Kind::ContainerShare),
        "--pid" | "--net" | "--network" | "--ipc" => {
            (value == "host").then_some(Kind::ContainerShare)
        }
        _ => None,
    }
}

/// 挂进去的宿主路径：`/`、`/dev`、`/sys` 整个的拒，它们下面的某一个问人。
fn mounted(source: &str) -> Option<Kind> {
    let trimmed = source.trim_end_matches('/');
    match trimmed {
        "" if source.starts_with('/') => Some(Kind::ContainerHost),
        "/dev" | "/sys" => Some(Kind::ContainerHost),
        path if path.starts_with("/dev/") || path.starts_with("/sys/") => {
            Some(Kind::ContainerShare)
        }
        _ => None,
    }
}
