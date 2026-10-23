//! `show` 认种类（施工 D-5，`docs/blueprint/tools/show.md`「怎么走」第 4 条）：图片、PDF、音视频照内容，svg、HTML、Markdown
//! 照后缀，别的 UTF-8 是文本（认得出语言的带上），认不出的二进制再照后缀认音视频（施工 D-5 补），再别的是文件。只给头选显示法，
//! 不给模型看。

use std::path::Path;

use miyu_tool::picture;

/// 认出来的：媒体类型、语言、图片的宽高。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Kind {
    pub(crate) media_type: &'static str,
    pub(crate) language: Option<&'static str>,
    pub(crate) size: Option<(u32, u32)>,
}

impl Kind {
    fn of(media_type: &'static str) -> Kind {
        Kind {
            media_type,
            language: None,
            size: None,
        }
    }
}

/// 认 `path`（真实的位置）的内容 `bytes`。
pub(crate) fn kind(path: &Path, bytes: &[u8]) -> Kind {
    let head = &bytes[..bytes.len().min(picture::HEAD)];
    if let Some(media_type) = picture::kind(head)
        && let Some(size) = picture::measure(bytes)
    {
        return Kind {
            size: Some(size),
            ..Kind::of(media_type)
        };
    }
    if bytes.starts_with(b"%PDF-") {
        return Kind::of("application/pdf");
    }
    if let Some(media_type) = media::by_content(&bytes[..bytes.len().min(media::HEAD)]) {
        return Kind::of(media_type);
    }
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase());
    match extension.as_deref() {
        Some("svg") => return Kind::of("image/svg+xml"),
        Some("html" | "htm") => return Kind::of("text/html"),
        Some("md" | "markdown") => return Kind::of("text/markdown"),
        _ => {}
    }
    if std::str::from_utf8(bytes).is_err() {
        return Kind::of(
            media::by_extension(extension.as_deref()).unwrap_or("application/octet-stream"),
        );
    }
    let name = path.file_name().map(|name| name.to_string_lossy());
    Kind {
        language: language(extension.as_deref(), name.as_deref()),
        ..Kind::of("text/plain")
    }
}

/// 后缀（小写）、文件名到 Markdown 代码块的语言名；认不出的没有。
fn language(extension: Option<&str>, name: Option<&str>) -> Option<&'static str> {
    match name {
        Some("Dockerfile") => return Some("dockerfile"),
        Some("Makefile") => return Some("makefile"),
        _ => {}
    }
    Some(match extension? {
        "rs" => "rust",
        "py" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" => "typescript",
        "tsx" => "tsx",
        "jsx" => "jsx",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "sh" | "bash" | "zsh" => "bash",
        "fish" => "fish",
        "ps1" => "powershell",
        "lua" => "lua",
        "sql" => "sql",
        "css" => "css",
        "scss" => "scss",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "xml" => "xml",
        "dart" => "dart",
        "scala" => "scala",
        "hs" => "haskell",
        "ex" | "exs" => "elixir",
        "r" => "r",
        "vim" => "vim",
        _ => return None,
    })
}

mod media;

#[cfg(test)]
mod tests;
