//! 门禁程序，和开发用的几样小工具（`cargo xtask dev-home`：造一个带配置的数据根，施工 8-6；`cargo xtask perf`：编好量尺
//! 跑一遍，施工 V-1）。`cargo xtask check` 依次跑格式、clippy、文档、三道门禁、许可证和测试，最后打一张结果表；测试跑在一个
//! 新的空临时目录里，跑完剩下的东西报在「临时目录」一项（施工 TMP-1，`temp.rs`）；这一项只在 Linux 上拦，macOS、Windows
//! 照数、照删、只报（主会话 2026-10-28 认了，原因见 `temp::ENFORCED`）。
//!
//! 三道门禁都照图纸查：`docs/designs/01-架构.md` 第九节「代码的分层」。「文档」那一项还查登记簿：
//! 给模型看的字和 `docs/designs/26-提示词.md` 第十节一一对上（施工 3-5 再补）；「给模型看的字」那一页和资源、登记簿
//! 对得上，蓝图里标着「样本」的块和它指到的文件一字不差（施工 4-9 三补）。`cargo xtask prompts` 重新生成那一页。
//! 「许可证」一项查发布的四个平台上用得到的第三方依赖，许可证都能和 GPL-3.0-or-later 合在一起发（施工 4-12）。
//! 里面的 cargo 一个接一个跑，不并行。

mod dev_home;
mod drawing;
mod files;
mod frozen;
mod heads;
mod layers;
mod ledger;
mod licenses;
mod logs;
mod perf;
mod prompts;
mod purity;
mod reference;
mod rights;
mod samples;
mod size;
mod temp;
mod test_targets;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use drawing::Drawing;
use layers::Package;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("check") => check(),
        Some("dev-home") => dev_home::run(std::env::args().nth(2).as_deref()),
        Some("perf") => {
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
            let extra: Vec<String> = std::env::args().skip(2).collect();
            perf::run(&cargo, &root(), &extra)
        }
        Some("logs") => {
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
            match logged(&cargo, &root()) {
                Ok(problems) if problems.is_empty() => {
                    println!("日志：没有要改的");
                    ExitCode::SUCCESS
                }
                Ok(problems) => {
                    problems.iter().for_each(|problem| println!("{problem}"));
                    println!("日志：{} 处", problems.len());
                    ExitCode::FAILURE
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("methods") => match frozen::add(&root()) {
            Ok(added) => {
                println!("登记了 {added} 个新方法进 {}", frozen::PATH);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        Some("prompts") => match prompts::write(&root()) {
            Ok(()) => {
                println!("写好了 {}", prompts::PATH);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!(
                "用法：cargo xtask check | cargo xtask logs | cargo xtask methods | cargo xtask prompts | cargo xtask dev-home <目录> | cargo xtask perf [量尺的参数]"
            );
            ExitCode::from(2)
        }
    }
}

/// 一项检查的结果。`problems` 是空的就算通过。
struct Outcome {
    name: &'static str,
    problems: Vec<String>,
}

/// 仓库的根目录。
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask 在仓库根目录下的 xtask/ 里，一定有上一级目录")
        .to_path_buf()
}

fn check() -> ExitCode {
    let root = root();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    let mut outcomes = vec![
        cargo_step(
            &cargo,
            &root,
            "格式",
            &["fmt", "--all", "--check"],
            &[],
            false,
        ),
        cargo_step(
            &cargo,
            &root,
            "clippy",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--quiet",
                "--",
                "-D",
                "warnings",
            ],
            &[],
            true,
        ),
        // 注释里的链接断了、写法坏了，生成文档时才看得出来，所以警告也当错。
        // 私有的也要生成：只生成公开的，私有代码注释里的断链查不出来。
        cargo_step(
            &cargo,
            &root,
            "文档",
            &[
                "doc",
                "--workspace",
                "--no-deps",
                "--document-private-items",
                "--quiet",
            ],
            &[("RUSTDOCFLAGS", "-D warnings")],
            true,
        ),
    ];
    println!("\n── 文档：登记簿 {} 第十节 ──", ledger::PATH);
    println!(
        "── 文档：{} 和资源、登记簿；蓝图里的样本块；{}；{} 和代码里的方法；{} 扩展能调什么的表；{} 协议 1 冻结的方法 ──",
        prompts::PATH,
        heads::PATH,
        reference::PATH,
        rights::PATH,
        frozen::PATH
    );
    if let Some(docs) = outcomes.last_mut() {
        docs.problems.extend(ledger::check(&root));
        docs.problems.extend(prompts::check(&root));
        docs.problems.extend(samples::check(&root));
        docs.problems.extend(heads::check(&root));
        docs.problems.extend(reference::check(&root));
        docs.problems.extend(rights::check(&root));
        docs.problems.extend(frozen::check(&root));
    }
    outcomes.extend(gates(&cargo, &root));
    println!(
        "\n── 许可证：cargo metadata，{} ──",
        licenses::PLATFORMS.join("、")
    );
    outcomes.push(Outcome {
        name: "许可证",
        problems: licenses::check(&root, &cargo),
    });
    // 先在锁里把测试程序都编好，再出锁跑（2026-10-22 项目主人定：锁只管编译）。编不过的不再跑。
    let mut tests = cargo_step(
        &cargo,
        &root,
        "测试",
        &["test", "--workspace", "--no-run", "--quiet"],
        &[],
        true,
    );
    let mut temp = Outcome {
        name: "临时目录",
        problems: Vec::new(),
    };
    if tests.problems.is_empty() {
        // 测试的临时目录指到一个新的空目录（施工 TMP-1）：跑完数剩下的、整个删掉。
        let dir = match temp::fresh() {
            Ok(dir) => Some(dir),
            Err(error) => {
                temp.problems.push(format!("建不了测试的临时目录：{error}"));
                None
            }
        };
        let shown = dir
            .as_ref()
            .map(|dir| dir.to_string_lossy().into_owned())
            .unwrap_or_default();
        let env: Vec<(&str, &str)> = match &dir {
            Some(_) => vec![("TMPDIR", &shown), ("TMP", &shown), ("TEMP", &shown)],
            None => Vec::new(),
        };
        tests = cargo_step(
            &cargo,
            &root,
            "测试",
            // 一条红不停（主会话 2026-10-26）：照旧跑完后面的 crate，别让一处盖掉别处的红。
            &["test", "--workspace", "--quiet", "--no-fail-fast"],
            &env,
            false,
        );
        if let Some(dir) = &dir {
            let left = temp::left(dir);
            match temp::ENFORCED {
                true => temp.problems.extend(left),
                // 只在 Linux 上拦（施工 TMP-1）：见 `temp::ENFORCED`。别的平台照数、照删，只报出来。
                false => left
                    .iter()
                    .for_each(|line| println!("临时目录（这个平台不拦）：{line}")),
            }
        }
    }
    println!("\n── 测试：每个测试文件都进了测试程序（autotests = false 的 crate）──");
    tests.problems.extend(test_crates(&cargo, &root));
    outcomes.push(tests);
    outcomes.push(temp);
    report(&outcomes)
}

/// 几个会话共用一台机器时的编译锁（2026-10-22 项目主人定）：设了这个环境变量（锁文件的路径）的，编译那几步照
/// `flock <锁文件>` 排队，跑测试、查格式这些不占锁。没设的照旧直接跑（CI、别的机器）。
const BUILD_LOCK: &str = "MIYU_BUILD_LOCK";

/// 跑一条 cargo 命令，输出照常打在终端上。`env` 是只加给这一条命令的环境变量，
/// 标题里照样打出来，方便照着手动重跑。`compiles` 的（要编译的那几步）照 [`BUILD_LOCK`] 排队。
fn cargo_step(
    cargo: &str,
    root: &Path,
    name: &'static str,
    args: &[&str],
    env: &[(&str, &str)],
    compiles: bool,
) -> Outcome {
    let shown: String = env.iter().map(|(k, v)| format!("{k}=\"{v}\" ")).collect();
    println!("\n── {name}：{shown}cargo {} ──", args.join(" "));
    let lock = std::env::var(BUILD_LOCK)
        .ok()
        .filter(|lock| compiles && !lock.is_empty());
    let mut command = match &lock {
        Some(lock) => {
            let mut flock = Command::new("flock");
            flock.arg(lock).arg(cargo);
            flock
        }
        None => Command::new(cargo),
    };
    let problems = match command
        .args(args)
        .envs(env.iter().copied())
        .current_dir(root)
        .status()
    {
        Ok(status) if status.success() => Vec::new(),
        Ok(status) => vec![format!(
            "cargo {} 没通过（{status}），原因见上面的输出",
            args[0]
        )],
        Err(e) => vec![format!("跑不了 cargo {}：{e}", args[0])],
    };
    Outcome { name, problems }
}

/// 照图纸、照源码查的四道门禁：分层、纯逻辑、行数、日志（施工 LG-1）。
fn gates(cargo: &str, root: &Path) -> Vec<Outcome> {
    let (drawing, packages) = match load(cargo, root) {
        Ok(loaded) => loaded,
        Err(e) => {
            return ["分层", "纯逻辑", "行数", "日志"]
                .into_iter()
                .map(|name| Outcome {
                    name,
                    problems: vec![e.clone()],
                })
                .collect();
        }
    };

    let mut purity = Vec::new();
    let mut size = Vec::new();
    let mut logged = Vec::new();
    for package in &packages {
        let pure = drawing.layer_of(&package.name).is_some_and(|l| l.pure);
        for (label, text) in sources(root, &package.dir, &mut size) {
            let in_src = Path::new(&label).starts_with(src_of(root, package));
            if pure && in_src {
                purity.extend(purity::scan(&label, &text));
            }
            if in_src {
                logged.extend(logs::scan(&label.replace('\\', "/"), &text));
            }
            size.extend(size::check(&label, &text, drawing.max_lines));
        }
    }
    vec![
        Outcome {
            name: "分层",
            problems: layers::check(&drawing, &packages),
        },
        Outcome {
            name: "纯逻辑",
            problems: purity,
        },
        Outcome {
            name: "行数",
            problems: size,
        },
        Outcome {
            name: "日志",
            problems: logged,
        },
    ]
}

/// 只扫日志（`cargo xtask logs`，施工 LG-1）：门禁「日志」那一项，单独跑得快，改日志的时候用。
fn logged(cargo: &str, root: &Path) -> Result<Vec<String>, String> {
    let packages = layers::read_packages(root, cargo)?;
    let mut problems = Vec::new();
    let mut unreadable = Vec::new();
    for package in &packages {
        for (label, text) in sources(root, &package.dir, &mut unreadable) {
            if Path::new(&label).starts_with(src_of(root, package)) {
                problems.extend(logs::scan(&label.replace('\\', "/"), &text));
            }
        }
    }
    problems.extend(unreadable);
    Ok(problems)
}

fn load(cargo: &str, root: &Path) -> Result<(Drawing, Vec<Package>), String> {
    let text = std::fs::read_to_string(root.join(drawing::PATH))
        .map_err(|e| format!("读不了图纸 {}：{e}", drawing::PATH))?;
    let drawing =
        drawing::parse(&text).map_err(|e| format!("图纸 {} 读不出来：{e}", drawing::PATH))?;
    let packages = layers::read_packages(root, cargo)?;
    Ok((drawing, packages))
}

/// 查每个 crate 的测试文件都进了测试程序（`test_targets`）；读不了 cargo metadata 的说一句。
fn test_crates(cargo: &str, root: &Path) -> Vec<String> {
    match layers::read_packages(root, cargo) {
        Ok(packages) => {
            let dirs: Vec<&Path> = packages
                .iter()
                .map(|package| package.dir.as_path())
                .collect();
            test_targets::check(root, &dirs)
        }
        Err(e) => vec![e],
    }
}

/// 一个 crate 的 `src/`，相对仓库根。
fn src_of(root: &Path, package: &Package) -> PathBuf {
    relative(root, &package.dir.join("src"))
}

fn relative(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

/// `dir` 下面每个 `.rs` 文件的相对路径和内容。读不了的记进 `problems`。
fn sources(root: &Path, dir: &Path, problems: &mut Vec<String>) -> Vec<(String, String)> {
    let files = match files::rust_files(dir) {
        Ok(files) => files,
        Err(e) => {
            problems.push(e);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for path in files {
        let label = relative(root, &path).display().to_string();
        match std::fs::read_to_string(&path) {
            Ok(text) => out.push((label, text)),
            Err(e) => problems.push(format!("读不了 {label}：{e}")),
        }
    }
    out
}

fn report(outcomes: &[Outcome]) -> ExitCode {
    println!("\n── 门禁结果 ──");
    for outcome in outcomes {
        if outcome.problems.is_empty() {
            println!("  ✓ {}", outcome.name);
        } else {
            println!("  ✗ {}：{} 处", outcome.name, outcome.problems.len());
            for problem in &outcome.problems {
                println!("      {problem}");
            }
        }
    }
    if outcomes.iter().all(|o| o.problems.is_empty()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
