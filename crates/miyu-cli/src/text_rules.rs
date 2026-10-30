//! 命令行给人看的字的检查（施工 CL-3，设计 47 第三节第 10 条）：扫帮助页、给人看的字的源文件，挡下写回去的老毛病。照文件扫，
//! 不跑命令：报错句末的句号由 [`crate::diagnostic`] 在运行时去掉，这里挡的是绕过它、直接往标准错误上写的。

use std::path::{Path, PathBuf};

/// 这个 crate 的 `src/`。
fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// `dir` 下扩展名是 `extension` 的文件，连子目录，照路径排。
fn files(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("读得了目录")
        .map(|entry| entry.expect("读得了").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            found.extend(files(&path, extension));
        } else if path.extension().is_some_and(|ext| ext == extension) {
            found.push(path);
        }
    }
    found
}

/// 测试文件：`tests.rs`、`tests/` 下的、`test_support.rs`。断言里写的是旧字、反例，不算。
fn is_test(path: &Path) -> bool {
    path.components().any(|part| part.as_os_str() == "tests")
        || path.file_name().is_some_and(|name| {
            name == "tests.rs" || name == "test_support.rs" || name == "text_rules.rs"
        })
}

/// 一行去掉 `//` 起的注释以后剩下的（只认行首的注释：给人看的字里本来就可能有 `//`，比如网址）。
fn code(line: &str) -> &str {
    if line.trim_start().starts_with("//") {
        ""
    } else {
        line
    }
}

/// 帮助页（`help/*/*.txt`）：缩进的那几行是命令、选项的说明，照动宾短语写，句末不带句号（第三节第 7 条）；省略号 `...` 是
/// 用法里的「可重复」，不算。中文帮助「命令：」下面那几行的说明不列「、」串（总表只写「管理知识库」，不列「列、搜、读、建」）。
#[test]
fn help_descriptions_have_no_full_stops_or_listed_actions() {
    let mut wrong = Vec::new();
    for page in files(&src().join("help"), "txt") {
        let text = std::fs::read_to_string(&page).expect("读得了");
        let mut commands = false;
        for (at, line) in text.lines().enumerate() {
            if !line.starts_with(' ') {
                commands = line == "命令：";
                continue;
            }
            let trimmed = line.trim_end();
            let stop =
                trimmed.ends_with('。') || (trimmed.ends_with('.') && !trimmed.ends_with("..."));
            if stop {
                wrong.push(format!(
                    "{}:{}: 句号结尾：{trimmed}",
                    page.display(),
                    at + 1
                ));
            }
            if commands && trimmed.contains('、') {
                wrong.push(format!(
                    "{}:{}: 命令说明列了一串：{trimmed}",
                    page.display(),
                    at + 1
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// 不用「她」，不写「你」（第三节第 1、8 条）：帮助页全文、源文件里不是注释的行（给人看的字都在源文件的字面量里）。
#[test]
fn no_her_or_you_in_what_people_read() {
    let mut wrong = Vec::new();
    let mut scan = |path: &Path, comments: bool| {
        let text = std::fs::read_to_string(path).expect("读得了");
        for (at, line) in text.lines().enumerate() {
            let line = if comments { code(line) } else { line };
            if line.contains('她') || line.contains('你') {
                wrong.push(format!("{}:{}: {}", path.display(), at + 1, line.trim()));
            }
        }
    };
    for page in files(&src().join("help"), "txt") {
        scan(&page, false);
    }
    for file in files(&src(), "rs")
        .into_iter()
        .filter(|path| !is_test(path))
    {
        scan(&file, true);
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// 报错一律带程序名（第三节第 1 条）：往标准错误上写字走 [`crate::diagnostic`]，别处不直接 `eprintln!`、`eprint!`。
#[test]
fn errors_go_through_the_diagnostic_module() {
    let mut wrong = Vec::new();
    for file in files(&src(), "rs")
        .into_iter()
        .filter(|path| !is_test(path))
    {
        if file.file_name().is_some_and(|name| name == "diagnostic.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&file).expect("读得了");
        for (at, line) in text.lines().enumerate() {
            let line = code(line);
            if line.contains("eprintln!") || line.contains("eprint!") {
                wrong.push(format!("{}:{}: {}", file.display(), at + 1, line.trim()));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// 出厂包的名字、说明也会印在 `miyu pkg search`、`miyu pkg info` 里（施工 CL-6）：不用「她」。注释不算。
#[test]
fn shipped_package_summaries_do_not_say_her() {
    let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/packages");
    let mut wrong = Vec::new();
    for manifest in files(&packages, "toml") {
        let text = std::fs::read_to_string(&manifest).expect("读得了");
        for (at, line) in text.lines().enumerate() {
            let line = if line.trim_start().starts_with('#') {
                ""
            } else {
                line
            };
            if line.contains('她') {
                wrong.push(format!(
                    "{}:{}: {}",
                    manifest.display(),
                    at + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
