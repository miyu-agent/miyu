//! `cargo xtask order-seed <目录>`（施工 MO-4，总施工单 `internal/construction/MO-macOS的发行包排函数.md` 第 3 条「谁提交」）：
//! 把一次打包产出的两份名字清单（`macos-miyu.txt`、`macos-miyu-tui.txt`，macOS 那一台 `cargo xtask dist` 写在
//! `<dist>/order/`，`release.yml` 把它作为构建产物传上去）收回仓库的 `xtask/order/`，做发版的会话在每次演练、正式发版以后
//! 用它。只做拷贝和格式检查（`seed.rs` 的 `problems`）：两份都过了才拷，有一份不对就一份都不拷。

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use super::seed;

/// 收回的两份清单的文件名，和仓库里 `xtask/order/` 下的一样。
const FILES: [&str; 2] = ["macos-miyu.txt", "macos-miyu-tui.txt"];

/// `cargo xtask order-seed <目录>`。
pub fn run(root: &Path, args: &[String]) -> ExitCode {
    let [from] = args else {
        eprintln!("用法：cargo xtask order-seed <放着 macos-miyu.txt、macos-miyu-tui.txt 的目录>");
        return ExitCode::from(2);
    };
    match adopt(Path::new(from), &root.join("xtask").join("order")) {
        Ok(lines) => {
            lines.iter().for_each(|line| println!("{line}"));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("order-seed：{error}");
            ExitCode::FAILURE
        }
    }
}

/// 查 `from` 里的两份清单，都过了拷进 `into`，交回每份说一行。
///
/// # Errors
///
/// 哪一份读不了、格式不对（一份都不拷）、写不进。
fn adopt(from: &Path, into: &Path) -> Result<Vec<String>, String> {
    let mut texts = Vec::new();
    for name in FILES {
        let file = from.join(name);
        let text = fs::read_to_string(&file)
            .map_err(|error| format!("读不了 {}（{error}）", file.display()))?;
        let problems = seed::problems(&text);
        if !problems.is_empty() {
            return Err(format!(
                "{} 的格式不对，一份都没拷：{}",
                file.display(),
                problems.join("；")
            ));
        }
        texts.push((name, text));
    }
    let mut lines = Vec::new();
    for (name, text) in texts {
        let file = into.join(name);
        let before = match fs::read_to_string(&file) {
            Ok(old) => super::verify::names(&old).len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => return Err(format!("读不了 {}（{error}）", file.display())),
        };
        fs::write(&file, &text).map_err(|error| format!("写不进 {}（{error}）", file.display()))?;
        lines.push(format!(
            "{}：{} 个名字（原来 {} 个）",
            file.display(),
            super::verify::names(&text).len(),
            before
        ));
    }
    Ok(lines)
}

#[cfg(test)]
mod tests;
