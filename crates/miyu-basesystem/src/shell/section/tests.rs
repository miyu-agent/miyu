//! 说明末尾讲脚本调工具的那一段（施工 M-4）：例子照 shell 和 `PATH` 挑，名单、样子照原样换进去，没有的那一行不带。

use std::ffi::OsString;
use std::path::PathBuf;

use super::*;

/// 出厂的资源目录。
fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

/// 一个临时目录，里面放着 `programs` 这几个空文件，当 `PATH` 用。
struct Bin(PathBuf);

impl Bin {
    fn with(programs: &[&str]) -> Bin {
        let dir = std::env::temp_dir().join(format!(
            "miyu-section-{}-{}",
            std::process::id(),
            programs.join("-")
        ));
        std::fs::create_dir_all(&dir).expect("建得了");
        for program in programs {
            std::fs::write(dir.join(program), b"").expect("写得进");
        }
        Bin(dir)
    }

    fn path(&self) -> OsString {
        self.0.clone().into_os_string()
    }
}

impl Drop for Bin {
    #[expect(clippy::let_underscore_must_use, reason = "删不掉的留在临时目录里")]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn powershell_gets_its_own_example_and_posix_shells_look_for_jq_then_python() {
    let none = Bin::with(&[]);
    assert_eq!(
        Example::pick(Kind::WindowsPowerShell, Some(&none.path())),
        Some(Example::PowerShell)
    );
    assert_eq!(
        Example::pick(Kind::PowerShell7, None),
        Some(Example::PowerShell)
    );
    let both = Bin::with(&["jq", "python3"]);
    assert_eq!(
        Example::pick(Kind::Bash, Some(&both.path())),
        Some(Example::Jq)
    );
    let python = Bin::with(&["python3"]);
    assert_eq!(
        Example::pick(Kind::Zsh, Some(&python.path())),
        Some(Example::Python)
    );
    assert_eq!(Example::pick(Kind::Bash, Some(&none.path())), None);
    assert_eq!(Example::pick(Kind::Bash, None), None);
}

#[test]
fn the_section_fills_names_example_and_shapes_as_they_are() {
    let section = Section::load(&resources(), Some(Example::Jq)).expect("读得出");
    let text = section.render("kb_read, web_search", r#"web_search {"links":[{"url"}]}"#);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    assert!(lines[0].starts_with("Commands and scripts you run here can call tools: "));
    assert!(
        lines[0].ends_with("Tools that work with miyu call: kb_read, web_search."),
        "{}",
        lines[0]
    );
    assert_eq!(
        lines[1],
        "Example: miyu call web_search --query niri | jq -r '.links[].site' | sort | uniq -c"
    );
    assert_eq!(
        lines[2], r#"JSON shapes: web_search {"links":[{"url"}]}. Other tools print text."#,
        "引号照原样，不转义"
    );
}

#[test]
fn no_example_and_no_shapes_leave_out_their_lines() {
    let section = Section::load(&resources(), None).expect("读得出");
    let text = section.render("web_fetch", "");
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.ends_with("Tools that work with miyu call: web_fetch."));
}

#[test]
fn every_example_loads_and_powershell_keeps_its_braces() {
    for example in [Example::PowerShell, Example::Jq, Example::Python] {
        let section = Section::load(&resources(), Some(example)).expect("读得出");
        let text = section.render("web_search", "");
        let line = text.lines().nth(1).expect("有例子那一行");
        assert!(line.starts_with("Example: miyu call web_search "), "{line}");
    }
    let section = Section::load(&resources(), Some(Example::PowerShell)).expect("读得出");
    assert!(
        section
            .render("web_search", "")
            .contains("{ $_.links.site }")
    );
}
