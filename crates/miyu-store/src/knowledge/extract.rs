//! 文档里的字（施工 K-5，`docs/blueprint/knowledge.md`「对外的样子」文档那一段、「怎么走」第二、五条）：Markdown、纯文本照原样
//! （得是 UTF-8）；PDF、网页交给小程序 `miyu-extract` 抽（主程序真实位置的旁边），一份文档拉起一次，抽完就退。抽字的两个库不编进
//! 核心：编进来空闲时多 1.6 MB 上下的常驻，过了设计 23 的预算（施工 K-5 量的）；抽坏文件卡住了也杀得掉。补、读共用这一套。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::catch_up::BIGGEST;

/// 抽字的小程序叫什么：Windows 上带 `.exe`。
const PROGRAM: &str = if cfg!(windows) {
    "miyu-extract.exe"
} else {
    "miyu-extract"
};

/// PDF、网页的文件最大几个字节：PDF 常带图，比文字的文档大得多；抽出来的字照旧以 [`BIGGEST`] 封顶。
pub(super) const BIGGEST_FILE: u64 = 64 * 1024 * 1024;

/// 一份最多等它抽多久：64 MiB 的 PDF 也够了，再久的当卡住，杀掉。
const PATIENCE: Duration = Duration::from_secs(60);

/// 等它的时候隔多久看一眼。
const GLANCE: Duration = Duration::from_millis(10);

/// Windows 上拉起它不弹黑框（同扩展进程）。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 主程序的真实位置 `exe` 旁边的 `miyu-extract`：没有的、不是普通文件的没有（那样的核心不认 PDF、网页，补的时候跳过）。
#[must_use]
pub fn beside(exe: &Path) -> Option<PathBuf> {
    let program = exe.parent()?.join(PROGRAM);
    program.is_file().then_some(program)
}

/// 认的文档的扩展名（小写），同 `Kind::of`（施工 K-7 修：命令行在传以前先看，说清只收哪几种；测试查着两边一样）。
pub const EXTENSIONS: [&str; 6] = ["md", "markdown", "txt", "pdf", "html", "htm"];

/// 一个文档是哪一种：照扩展名（小写比）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// `.md`、`.markdown`：照标题切。
    Markdown,
    /// `.txt`：照段落切。
    Plain,
    /// `.pdf`：一页一页抽，照页切。
    Pdf,
    /// `.html`、`.htm`：换成文字，照标题切。
    Html,
}

impl Kind {
    /// 文件名 `name` 是哪一种文档；扩展名不认的、只有扩展名的（`.md`）不是文档。
    pub(super) fn of(name: &str) -> Option<Kind> {
        let (stem, extension) = name.rsplit_once('.')?;
        if stem.is_empty() {
            return None;
        }
        match extension.to_ascii_lowercase().as_str() {
            "md" | "markdown" => Some(Kind::Markdown),
            "txt" => Some(Kind::Plain),
            "pdf" => Some(Kind::Pdf),
            "html" | "htm" => Some(Kind::Html),
            _ => None,
        }
    }

    /// 她写得了的：文字的两种（PDF、网页照原样存着，写进字会弄坏）。
    pub(super) fn writable(self) -> bool {
        matches!(self, Kind::Markdown | Kind::Plain)
    }

    /// 文件最大几个字节。
    pub(super) fn biggest(self) -> u64 {
        match self {
            Kind::Markdown | Kind::Plain => BIGGEST,
            Kind::Pdf | Kind::Html => BIGGEST_FILE,
        }
    }
}

/// 文件 `file`（`kind` 那一种）里的字：PDF 一页一份，别的只有一份。PDF、网页照 `extractor` 抽，没有的抽不了。抽不出来的交回
/// 为什么（英文的一句）。
pub(super) fn pages(
    kind: Kind,
    file: &Path,
    extractor: Option<&Path>,
) -> Result<Vec<String>, String> {
    let program = match kind {
        Kind::Markdown | Kind::Plain => {
            let bytes = std::fs::read(file).map_err(|error| error.to_string())?;
            return String::from_utf8(bytes)
                .map(|text| vec![text])
                .map_err(|_| "not UTF-8".to_string());
        }
        Kind::Pdf | Kind::Html => extractor.ok_or("no miyu-extract beside the main program")?,
    };
    let kind = if kind == Kind::Pdf { "pdf" } else { "html" };
    let (stdout, stderr) = run(program, kind, file)?;
    match stdout {
        Some(stdout) => serde_json::from_slice(&stdout).map_err(|error| error.to_string()),
        None => Err(String::from_utf8_lossy(&stderr).trim().to_string()),
    }
}

/// 拉起 `program` 抽 `file`，等它退：交回抽成了的标准输出（退出码不是 0 的没有）和标准错误。等过了 [`PATIENCE`] 的杀掉。
fn run(program: &Path, kind: &str, file: &Path) -> Result<(Option<Vec<u8>>, Vec<u8>), String> {
    let mut command = Command::new(program);
    command
        .arg(kind)
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("{}: {error}", program.display()))?;
    // 两头各一个线程读到底：只读一头的话，另一头写满了管道它就卡住。
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = pipe {
                // 读到一半断了的照读到的算：退出码说了算成没成。
                let _partial = pipe.read_to_end(&mut bytes);
            }
            bytes
        })
    };
    let stdout = drain(
        child
            .stdout
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );
    let stderr = drain(
        child
            .stderr
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );
    let deadline = Instant::now() + PATIENCE;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(GLANCE),
            Ok(None) => {
                // 杀不掉、收不回的只能由它去：照实报卡住了。
                let _killed = child.kill();
                let _reaped = child.wait();
                return Err(format!("took longer than {} s", PATIENCE.as_secs()));
            }
            Err(error) => return Err(error.to_string()),
        }
    };
    let stdout = stdout.join().map_err(|_| "reading the output failed")?;
    let stderr = stderr.join().map_err(|_| "reading the output failed")?;
    Ok((status.success().then_some(stdout), stderr))
}
