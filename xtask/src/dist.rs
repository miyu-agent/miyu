//! `cargo xtask dist`（施工 RL-1，设计 `59-发版.md` 第三节第 1 条、第六节第 1 条）：打命令行版的压缩包，本机打本机那一种。
//!
//! 1. 版本照 `Cargo.toml` 的 `[workspace.package] version`；系统照本机（`linux-x86_64`、`linux-aarch64`、`macos-aarch64`、
//!    `windows-x86_64`），写了 `--target` 的要和本机一样，不交叉编。
//! 2. 照 `heads.toml` 把两个头取进 `<dist>/heads/<编号>/`（克隆过的拉一下），检出钉的那一份、清掉多余的文件，核对检出的提交。
//! 3. 编，都是 release、`--locked`：核心的 `miyu`、`miyu-sandbox`、`miyu-extract`；终端的 `miyu-tui`；网页的 `miyu-web`。头在各自的
//!    检出里照各自的工具链编：外面这个 cargo 给的 `RUSTUP_TOOLCHAIN`、`CARGO_TARGET_DIR` 这些不带过去。
//! 4. 搭目录 `<dist>/miyu-<版本>-<系统>/`：
//!    - 五个程序放一起（头的程序放在 `miyu` 旁边，`heads.toml` 开头那段）；
//!    - `resources/`：仓库里登记了的资源（`git ls-files`，不带没登记的），加上两个头的包文件夹：`packages/miyu-tui/` 是终端仓库的
//!      `miyu-tui/` 加它的 `LICENSE`、`THIRD-PARTY-LICENSES.txt`（同它的发版模板），`packages/miyu-webui/` 是网页仓库
//!      `tools/pack-web.py --stage` 搭的、去掉 `bin/`；
//!    - `LICENSE`；`THIRD-PARTY-LICENSES.txt` 合核心、终端、网页三份（`packaging/README.md`：带了头的，头的那一份也放进去）；
//!    - `README.txt`：中英两段，怎么用、装在哪、怎么卸。
//! 5. 打成 `miyu-<版本>-<系统>.tar.gz`（Windows 是 `.zip`），里面一层同名目录，照名字排、不带属主和时刻；印路径、SHA-256、大小。
//!
//! `<dist>` 默认是 cargo 的 target 目录下的 `dist/`，`--out` 换地方。查包、签名、Release 是 RL-2 的事。

mod archive;
pub mod check;
mod fetch;
mod layout;
mod split;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use toml_edit::DocumentMut;

use crate::heads;
use archive::{digest, pack};
use fetch::{fetch, head_cargo, python, status, target_dir, tracked};
use layout::{Sources, copy_tree, notices, readme, remove, stage};
#[cfg(test)]
use {crate::heads::Pin, layout::executable};

/// 本机是哪一种系统，发的四种以外的是 `None`。
pub const HOST: Option<&str> = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    Some("linux-x86_64")
} else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
    Some("linux-aarch64")
} else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
    Some("macos-aarch64")
} else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
    Some("windows-x86_64")
} else {
    None
};

/// 核心的三个程序，照这个先后编、放。
const CORE: [&str; 3] = ["miyu", "miyu-sandbox", "miyu-extract"];

/// 终端、网页的程序。
const TUI_PROGRAM: &str = "miyu-tui";
const WEB_PROGRAM: &str = "miyu-web";

/// 命令行：打给哪一种、放在哪。
#[derive(Debug, PartialEq, Eq)]
struct Options {
    target: String,
    out: Option<PathBuf>,
}

/// 打好的一个压缩包。
#[derive(Debug)]
struct Packed {
    path: PathBuf,
    sha256: String,
    bytes: u64,
}

/// `cargo xtask dist [--target <系统>] [--out <目录>]`。
pub fn run(root: &Path, cargo: &str, args: &[String]) -> ExitCode {
    match dist(root, cargo, args) {
        Ok(all) => {
            for packed in all {
                println!("打好了 {}", packed.path.display());
                println!("SHA-256：{}", packed.sha256);
                println!("大小：{} 字节", packed.bytes);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("dist：{error}");
            ExitCode::FAILURE
        }
    }
}

fn dist(root: &Path, cargo: &str, args: &[String]) -> Result<Vec<Packed>, String> {
    let options = options(args, HOST)?;
    let version = version(&read(&root.join("Cargo.toml"))?)?;
    let pins = heads::pins(&read(&root.join(heads::PATH))?).map_err(|found| found.join("\n"))?;
    let target_dir = target_dir(root, cargo)?;
    let work = options
        .out
        .clone()
        .unwrap_or_else(|| target_dir.join("dist"));
    let exe = std::env::consts::EXE_SUFFIX;

    let mut checkouts = Vec::new();
    for pin in &pins {
        let dir = work.join("heads").join(&pin.id);
        say(&format!("取 {} {}", pin.id, pin.rev));
        fetch(&dir, pin)?;
        checkouts.push(dir);
    }
    let [tui, web] = checkouts.as_slice() else {
        return Err(format!("{} 里要正好两个头", heads::PATH));
    };

    say("编核心");
    let mut core = Command::new(cargo);
    core.current_dir(root)
        .args(["build", "--release", "--locked"])
        .args(CORE.iter().flat_map(|name| ["-p", name]));
    status(&mut core)?;
    say("编终端");
    status(head_cargo(tui).args(["build", "--release", "--locked"]))?;
    say("编网页");
    status(head_cargo(web).args(["build", "--release", "--locked", "-p", WEB_PROGRAM]))?;
    let web_binary = web
        .join("target/release")
        .join(format!("{WEB_PROGRAM}{exe}"));
    let web_stage = work.join("heads").join("miyu-webui-stage");
    remove(&web_stage)?;
    say("搭网页的包文件夹");
    status(
        python()?
            .current_dir(web)
            .arg("tools/pack-web.py")
            .arg("--stage")
            .arg(&web_stage)
            .arg("--binary")
            .arg(&web_binary)
            .arg("--target")
            .arg(&options.target),
    )?;

    let release = target_dir.join("release");
    let mut programs: Vec<PathBuf> = CORE
        .iter()
        .map(|name| release.join(format!("{name}{exe}")))
        .collect();
    programs.push(
        tui.join("target/release")
            .join(format!("{TUI_PROGRAM}{exe}")),
    );
    programs.push(web_binary);
    let core_programs: Vec<PathBuf> = programs[..CORE.len()].to_vec();
    let tui_program = programs[CORE.len()].clone();
    let mut tui_package: Vec<(PathBuf, PathBuf)> = tracked(tui, "miyu-tui")?
        .into_iter()
        .map(|file| {
            let inside = file.strip_prefix("miyu-tui").unwrap_or(&file).to_path_buf();
            (tui.join(&file), inside)
        })
        .collect();
    for name in ["LICENSE", "THIRD-PARTY-LICENSES.txt"] {
        tui_package.push((tui.join(name), PathBuf::from(name)));
    }
    let resources = root.join("resources");
    let resource_files: Vec<PathBuf> = tracked(root, "resources")?
        .into_iter()
        .map(|file| {
            file.strip_prefix("resources")
                .unwrap_or(&file)
                .to_path_buf()
        })
        .collect();
    let core_notices = read(&root.join("THIRD-PARTY-LICENSES.txt"))?;
    let notices = notices(
        &core_notices,
        (&pins[0], &read(&tui.join("THIRD-PARTY-LICENSES.txt"))?),
        (
            &pins[1],
            &read(&web_stage.join("THIRD-PARTY-LICENSES.txt"))?,
        ),
    );
    let sources = Sources {
        programs,
        resources: resources.clone(),
        resource_files: resource_files.clone(),
        tui_package: tui_package.clone(),
        web_package: web_stage.clone(),
        license: root.join("LICENSE"),
        notices,
        readme: readme(&version, &options.target),
    };
    let name = format!("miyu-{version}-{}", options.target);
    let dir = work.join(&name);
    say(&format!("搭 {}", dir.display()));
    remove(&dir)?;
    stage(&sources, &dir)?;
    let archive = work.join(archive_name(&version, &options.target));
    say("打压缩包");
    pack(&dir, &name, &archive)?;
    let mut all = vec![packed(archive)?];

    // 系统源分包用的三份（施工 SD-2）：Windows 不出。
    if !split::wanted(&options.target) {
        say("这台不出分包（没有系统源）");
        return Ok(all);
    }
    let suffix = "tar.gz";
    let core = split::core_name(&version, &options.target);
    let core_dir = work.join(&core);
    say(&format!("搭 {}", core_dir.display()));
    remove(&core_dir)?;
    split::stage_core(
        &core_programs,
        (&resources, &resource_files),
        &root.join("LICENSE"),
        &core_notices,
        &core_dir,
    )?;
    let core_archive = work.join(format!("{core}.{suffix}"));
    pack(&core_dir, &core, &core_archive)?;
    all.push(packed(core_archive)?);

    let tui_version = split::version_of(&read(&tui.join("miyu-tui/package.toml"))?)?;
    let web_version = split::version_of(&read(&web_stage.join("package.toml"))?)?;
    for (id, head_version) in [("miyu-tui", &tui_version), ("miyu-webui", &web_version)] {
        // 包文件夹叫编号，放在一个单独的目录下面：压缩包里那一层就是它。
        let holder = work.join("split").join(id);
        remove(&holder)?;
        let folder = holder.join(id);
        say(&format!("搭 {}", folder.display()));
        match id {
            "miyu-tui" => split::stage_head(&tui_package, &tui_program, &folder)?,
            _ => copy_tree(&web_stage, &folder, &[])?,
        }
        let name = split::head_name(id, head_version, &options.target);
        let archive = work.join(format!("{name}.{suffix}"));
        pack(&folder, id, &archive)?;
        all.push(packed(archive)?);
    }
    Ok(all)
}

/// 打好的一个压缩包：算 SHA-256、字节数。
fn packed(path: PathBuf) -> Result<Packed, String> {
    let (sha256, bytes) = digest(&path)?;
    Ok(Packed {
        path,
        sha256,
        bytes,
    })
}

/// 读命令行。不写 `--target` 的照本机；写了的要和本机一样（不交叉编）。
fn options(args: &[String], host: Option<&str>) -> Result<Options, String> {
    let Some(host) = host else {
        return Err("本机不是发的四种系统之一（linux-x86_64、linux-aarch64、macos-aarch64、windows-x86_64）".to_string());
    };
    let mut target = None;
    let mut out = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--target" => target = Some(rest.next().ok_or("--target 后面要写系统")?.clone()),
            "--out" => out = Some(PathBuf::from(rest.next().ok_or("--out 后面要写目录")?)),
            other => {
                return Err(format!(
                    "不认识的参数 {other}；用法：cargo xtask dist [--target <系统>] [--out <目录>]"
                ));
            }
        }
    }
    let target = target.unwrap_or_else(|| host.to_string());
    if target != host {
        return Err(format!(
            "只打本机那一种（{host}），写的是 {target}：别的系统在那种系统上打"
        ));
    }
    Ok(Options { target, out })
}

/// `Cargo.toml` 的 `[workspace.package] version`。
fn version(manifest: &str) -> Result<String, String> {
    let doc: DocumentMut = manifest
        .parse()
        .map_err(|error| format!("Cargo.toml 不是 TOML（{error}）"))?;
    doc.get("workspace")
        .and_then(|item| item.get("package"))
        .and_then(|item| item.get("version"))
        .and_then(|item| item.as_str())
        .map(str::to_string)
        .ok_or_else(|| "Cargo.toml 的 [workspace.package] 里没有 version".to_string())
}

/// 压缩包的名字：`miyu-<版本>-<系统>.tar.gz`，Windows 的是 `.zip`。
fn archive_name(version: &str, target: &str) -> String {
    let suffix = if target.starts_with("windows") {
        "zip"
    } else {
        "tar.gz"
    };
    format!("miyu-{version}-{target}.{suffix}")
}

fn say(what: &str) {
    println!("== {what}");
}

fn read(file: &Path) -> Result<String, String> {
    fs::read_to_string(file).map_err(|error| format!("{}：{error}", file.display()))
}

#[cfg(test)]
mod tests;
