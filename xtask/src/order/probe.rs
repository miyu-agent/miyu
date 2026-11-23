//! macOS 打包时带探针跑的一轮（施工 MO-4，总施工单 `internal/construction/MO-macOS的发行包排函数.md` 第 1、3、5、7 条，
//! `internal/blueprint/perf.md`「排函数」的 macOS 一款）：这一轮不改这一次的程序，只写出给下一次用的起点清单。
//!
//! 1. 编任何东西之前（[`build`]）：用真的 `cc` 编页探针（`crates/miyu-perf/probe/pagetrace.c`）到
//!    `<target>/order/pagetrace.dylib`，编法同 macOS 的 CI（`-Wall -Wextra -Werror`）。
//! 2. 搭好发行包目录、核对过以后（[`round`]）：
//!    1. 记下目录里 `miyu`、`miyu-tui` 的 SHA-256；
//!    2. 编量尺 `miyu-perf`，带核心编译时的同一个 `--config`（不设 `MIYU_ORDER_NAMES`，垫片原样调 `cc`）：链接器的参数
//!       一样，共同的依赖不用重编；
//!    3. 跑 `miyu-perf --order … --probe <探针> --head <目录>/miyu-tui --head-order …`，五段对着发行包目录里要发出去的两个
//!       程序。探针是运行时插进去的，不改程序文件；
//!    4. 核对两个程序和跑之前逐字节一样，目录里没有探针的动态库（[`clean`]：照文件名，也照文件里有没有探针的安装名，即
//!       Mach-O 的 `LC_ID_DYLIB`，编的时候就是它的输出路径）；
//!    5. 两个程序各自合并（`seed.rs` 的 `merge`），写进 `<dist>/order/macos-miyu.txt`、`macos-miyu-tui.txt`，说一行新看到
//!       几个、去掉几个；新看到的太多（[`TOO_MANY`]）时提醒起点清单太旧，不算失败。
//!
//! 哪一步不成就报错停下，报错里写临时怎么绕过：`cargo xtask dist --no-probe` 照起点清单排、照样核对，只是不跑这一轮、
//! 不写新清单（总施工单第 7 条）。

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use sha2::{Digest, Sha256};

use super::link::CC;
use super::macos::{Ordered, Shims};
use super::seed::{self, Touched};
use super::{Recording, recording, verify};
use crate::dist::{output, status};

/// 探针的源码，相对仓库。
const SOURCE: &str = "crates/miyu-perf/probe/pagetrace.c";

/// 探针编出来的文件名。
const LIBRARY: &str = "pagetrace.dylib";

/// 一轮里新看到的清单外函数超过这么多时提醒（核心、终端界面）：实验里起点清单收敛以后核心停在 15 个左右、终端界面
/// 3 个，照 Linux 清单对过来的第一轮是 167、30 个（总施工单第 2 条「发版时只跑一轮」）。
const TOO_MANY: (usize, usize) = (200, 50);

/// 跑不下去时附在报错后面的一句。
const BYPASS: &str =
    "临时绕过可以加 --no-probe：照起点清单排、照样核对，只是不跑探针、不写新的起点清单";

/// 编页探针，交回动态库的绝对路径（见模块的说明第 1 条）。
///
/// # Errors
///
/// `cc` 起不来、编不过、目录建不了。
pub(crate) fn build(root: &Path, target: &Path) -> Result<PathBuf, String> {
    let dir = target.join("order");
    fs::create_dir_all(&dir).map_err(|error| format!("建不了 {}（{error}）", dir.display()))?;
    let library = dir.join(LIBRARY);
    status(
        Command::new(CC)
            .args(["-O2", "-dynamiclib", "-Wall", "-Wextra", "-Werror", "-o"])
            .arg(&library)
            .arg(root.join(SOURCE)),
    )
    .map_err(|error| format!("编不过页探针：{error}。{BYPASS}"))?;
    Ok(library)
}

/// 跑一轮要的几处。
pub(crate) struct Round<'a> {
    /// 仓库。
    pub(crate) root: &'a Path,
    /// 外面的 cargo。
    pub(crate) cargo: &'a str,
    /// 核心的 cargo target 目录。
    pub(crate) target: &'a Path,
    /// 搭好、核对过的发行包目录。
    pub(crate) dir: &'a Path,
    /// 新的起点清单写进哪个目录（`<dist>/order/`）。
    pub(crate) out: &'a Path,
    /// 探针的动态库（[`build`]）。
    pub(crate) library: &'a Path,
    /// 两个程序的起点清单、垫片。
    pub(crate) shims: &'a Shims,
}

/// 带探针跑一轮、合并、写出下一次的起点清单（见模块的说明第 2 条）。`say` 同 `dist.rs`。
///
/// # Errors
///
/// 量尺编不过、跑不成（探针一条都没记到、剧本某一段等不到）、跑完程序变了或者目录里有探针、清单读写不了。
pub(crate) fn round(round: &Round<'_>, say: &dyn Fn(&str)) -> Result<(), String> {
    let started = Instant::now();
    let programs = [
        (&round.shims.core, "miyu", TOO_MANY.0),
        (&round.shims.tui, "miyu-tui", TOO_MANY.1),
    ];
    let before = programs
        .iter()
        .map(|(_, name, _)| digest(&round.dir.join(name)))
        .collect::<Result<Vec<_>, _>>()?;

    say("排函数：编量尺（带同一个垫片，不排）");
    let building = Instant::now();
    let mut perf = Command::new(round.cargo);
    perf.current_dir(round.root)
        .args(["build", "--release", "--locked", "-p", "miyu-perf"]);
    status(round.shims.core.linker(&mut perf)?).map_err(|error| format!("编量尺：{error}"))?;
    say(&format!(
        "排函数：编量尺用了 {} 秒",
        building.elapsed().as_secs()
    ));

    let lists = round.target.join("order").join("macos");
    let probed = programs.map(|(_, name, _)| lists.join(format!("probe-{name}.txt")));
    for list in &probed {
        for file in [list.clone(), touched(list)] {
            if file.exists() {
                fs::remove_file(&file)
                    .map_err(|error| format!("删不掉上一次的 {}（{error}）", file.display()))?;
            }
        }
    }
    say(&format!(
        "排函数：带探针对着 {} 跑一轮（五段）",
        round.dir.display()
    ));
    let running = Instant::now();
    let (miyu, resources) = (round.dir.join("miyu"), round.dir.join("resources"));
    let (work, tree) = (round.target.join("perf-order"), round.root.join("crates"));
    let paths = Recording {
        miyu: &miyu,
        resources: &resources,
        work: &work,
        tree: &tree,
        list: &probed[0],
        flag: "--probe",
        recorder: round.library,
    };
    let mut command = recording(&round.target.join("release").join("miyu-perf"), &paths);
    command
        .arg("--head")
        .arg(round.dir.join("miyu-tui"))
        .arg("--head-order")
        .arg(&probed[1]);
    status(command.current_dir(round.root))
        .map_err(|error| format!("探针那一轮：{error}。{BYPASS}"))?;
    say(&format!(
        "排函数：探针那一轮用了 {} 秒",
        running.elapsed().as_secs()
    ));

    for ((_, name, _), was) in programs.iter().zip(&before) {
        let program = round.dir.join(name);
        if digest(&program)? != *was {
            return Err(format!(
                "跑完探针那一轮，{} 和跑之前不一样了：探针不该改程序文件",
                program.display()
            ));
        }
    }
    clean(round.dir, round.library)?;
    say("排函数：两个程序和跑之前逐字节一样，发行包目录里没有探针");

    fs::create_dir_all(round.out)
        .map_err(|error| format!("建不了 {}（{error}）", round.out.display()))?;
    for ((ordered, name, limit), list) in programs.iter().zip(&probed) {
        let written = merged(ordered, &round.dir.join(name), list, round.out, name)?;
        say(&format!(
            "排函数：{}本轮新看到 {} 个清单外的函数，去掉 {} 个没被碰过的页上的名字（对不上符号、留着的 {} 个），下一次的起点\
             清单 {} 个名字（{}）",
            ordered.label(),
            written.added,
            written.removed,
            written.unmatched,
            written.names.len(),
            round.out.join(format!("macos-{name}.txt")).display()
        ));
        if written.added > *limit {
            say(&format!(
                "排函数：提醒：{}新看到的清单外函数超过 {limit} 个，起点清单太旧，这次排的效果打了折扣",
                ordered.label()
            ));
        }
    }
    say(&format!(
        "排函数：探针那一轮连编量尺、合并一共用了 {} 秒",
        started.elapsed().as_secs()
    ));
    Ok(())
}

/// 合并一个程序：读起点清单、量尺这一轮的清单和碰过的页、发行包目录里程序的符号表，写出 `<out>/macos-<name>.txt`。
fn merged(
    ordered: &Ordered,
    program: &Path,
    list: &Path,
    out: &Path,
    name: &str,
) -> Result<seed::Merged, String> {
    let read = |file: &Path| {
        fs::read_to_string(file).map_err(|error| format!("读不了 {}（{error}）", file.display()))
    };
    let seen = read(list)?;
    let pages = touched(list);
    let touched = Touched::parse(&read(&pages)?)
        .map_err(|error| format!("{} 读不懂：{error}", pages.display()))?;
    let symbols = output(
        Command::new("nm")
            .args(["--defined-only", "--numeric-sort", "-S"])
            .arg(program),
    )?;
    let seen: Vec<&str> = verify::names(&seen);
    let written = seed::merge(
        &read(ordered.seed())?,
        &seen,
        &verify::sized(&symbols),
        &touched,
    );
    let text = seed::render(&written.names.iter().map(String::as_str).collect::<Vec<_>>());
    let problems = seed::problems(&text);
    if !problems.is_empty() {
        return Err(format!(
            "合并出来的{}起点清单格式不对：{}",
            ordered.label(),
            problems.join("；")
        ));
    }
    let file = out.join(format!("macos-{name}.txt"));
    fs::write(&file, text).map_err(|error| format!("写不进 {}（{error}）", file.display()))?;
    Ok(written)
}

/// 量尺在清单 `list` 旁边写的碰过的页：`<清单>.pages`（同 `crates/miyu-perf/src/order/pages.rs` 的 `touched_file`）。
fn touched(list: &Path) -> PathBuf {
    let mut name = OsString::from(list.as_os_str());
    name.push(".pages");
    PathBuf::from(name)
}

/// 核对目录 `dir`（含子目录）里没有探针的动态库 `library`：没有和它同名的文件，也没有哪个文件里写着它的安装名（它的
/// 绝对路径）。打压缩包以前对每个要打的目录都核对一次（core1 审总施工单时补的第 1 条）。
///
/// # Errors
///
/// 目录读不了、找到了探针。
pub(crate) fn clean(dir: &Path, library: &Path) -> Result<(), String> {
    let needle = library.as_os_str().as_encoded_bytes();
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(at) = pending.pop() {
        let unreadable = |error: std::io::Error| format!("读不了 {}（{error}）", at.display());
        for entry in fs::read_dir(&at).map_err(unreadable)? {
            let path = entry.map_err(unreadable)?.path();
            let kind = fs::symlink_metadata(&path)
                .map_err(|error| format!("读不了 {}（{error}）", path.display()))?;
            if kind.is_dir() {
                pending.push(path);
                continue;
            }
            let named = path.file_name().is_some_and(|name| name == LIBRARY);
            let bytes =
                fs::read(&path).map_err(|error| format!("读不了 {}（{error}）", path.display()))?;
            if named || bytes.windows(needle.len()).any(|window| window == needle) {
                found.push(path.display().to_string());
            }
        }
    }
    match found.is_empty() {
        true => Ok(()),
        false => Err(format!(
            "{} 里有探针的动态库（{}），它不许进发行包：{}",
            dir.display(),
            library.display(),
            found.join("、")
        )),
    }
}

/// 文件的 SHA-256。
fn digest(file: &Path) -> Result<Vec<u8>, String> {
    let bytes = fs::read(file).map_err(|error| format!("读不了 {}（{error}）", file.display()))?;
    Ok(Sha256::digest(&bytes).to_vec())
}

#[cfg(test)]
mod tests;
