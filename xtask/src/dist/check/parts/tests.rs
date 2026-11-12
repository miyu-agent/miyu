//! 分包的查法里不跑程序的几步：认文件名、核心那一份的内容、头那一份的包文件夹。

use super::*;
use crate::dist::layout::executable;

/// 测试的临时目录，用完删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("xtask-parts-{name}-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("清得掉上次留下的");
        }
        fs::create_dir_all(&dir).expect("建得了");
        Scratch(dir)
    }

    fn put(&self, file: &str, text: &str) -> PathBuf {
        let path = self.0.join(file);
        fs::create_dir_all(path.parent().expect("有上一级")).expect("建得了");
        fs::write(&path, text).expect("写得进");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("临时目录没删掉：{error}");
        }
    }
}

#[test]
fn head_archives_are_named_by_id_version_and_system() {
    let t = "linux-x86_64";
    assert_eq!(
        head_of("tui-0.1.0-linux-x86_64.tar.gz", t),
        Some(("tui", "0.1.0".to_string()))
    );
    assert_eq!(
        head_of("webui-0.2.0-rc.1-linux-x86_64.tar.gz", t),
        Some(("webui", "0.2.0-rc.1".to_string()))
    );
    assert_eq!(head_of("tui--linux-x86_64.tar.gz", t), None, "没有版本");
    assert_eq!(
        head_of("tui-0.1.0-macos-aarch64.tar.gz", t),
        None,
        "别的系统"
    );
    assert_eq!(head_of("miyu-core-0.0.0-linux-x86_64.tar.gz", t), None);
    assert_eq!(head_of("tui-0.1.0-linux-x86_64.zip", t), None);
}

/// 一个仓库（`repo/`）和照它搭的核心那一份（`pkg/`）。
fn core(scratch: &Scratch) -> Vec<PathBuf> {
    scratch.put("repo/LICENSE", "agpl");
    scratch.put("repo/THIRD-PARTY-LICENSES.txt", "core notices\n");
    scratch.put("repo/resources/core/a.md", "a");
    for program in CORE {
        let file = scratch.put(
            &format!("pkg/{program}{}", std::env::consts::EXE_SUFFIX),
            "p",
        );
        executable(&file).unwrap();
    }
    scratch.put("pkg/LICENSE", "agpl");
    scratch.put("pkg/THIRD-PARTY-LICENSES.txt", "core notices\n");
    scratch.put("pkg/resources/core/a.md", "a");
    vec![PathBuf::from("resources/core/a.md")]
}

fn core_problems(scratch: &Scratch, resources: &[PathBuf]) -> Vec<String> {
    core_contents(&scratch.0.join("repo"), &scratch.0.join("pkg"), resources).unwrap()
}

#[test]
fn the_core_part_matches_the_repository_without_the_heads() {
    let scratch = Scratch::new("core-good");
    let resources = core(&scratch);
    assert_eq!(core_problems(&scratch, &resources), Vec::<String>::new());
}

/// 一种弄坏的法子：名字、怎么弄坏、报的话里该有的字。
type Spoil = (&'static str, fn(&Scratch), &'static str);

#[test]
fn every_core_part_mistake_is_named() {
    let cases: [Spoil; 6] = [
        (
            "program",
            |s| {
                fs::remove_file(
                    s.0.join(format!("pkg/miyu-extract{}", std::env::consts::EXE_SUFFIX)),
                )
                .unwrap()
            },
            "少了程序 miyu-extract",
        ),
        (
            "head-program",
            |s| {
                drop(s.put(
                    &format!("pkg/miyu-tui{}", std::env::consts::EXE_SUFFIX),
                    "x",
                ))
            },
            "不该带 miyu-tui",
        ),
        (
            "head-folder",
            |s| drop(s.put("pkg/resources/packages/webui/package.toml", "x")),
            "不该有 webui",
        ),
        (
            "notices",
            |s| drop(s.put("pkg/THIRD-PARTY-LICENSES.txt", "joined\n")),
            "核心那一份",
        ),
        ("license", |s| drop(s.put("pkg/LICENSE", "mit")), "LICENSE"),
        (
            "changed",
            |s| drop(s.put("pkg/resources/core/a.md", "b")),
            "和仓库的不一样",
        ),
    ];
    for (name, spoil, said) in cases {
        let scratch = Scratch::new(&format!("core-{name}"));
        let resources = core(&scratch);
        spoil(&scratch);
        let found = core_problems(&scratch, &resources);
        assert!(
            found.iter().any(|one| one.contains(said)),
            "{name}：{found:?}"
        );
    }
}

/// 一个头的包文件夹：`id`、版本 `version`、程序在 `bin/`。
fn head(scratch: &Scratch, id: &str, program: &str, version: &str) -> PathBuf {
    scratch.put(
        &format!("{id}/package.toml"),
        &format!("[package]\nversion = \"{version}\"\n"),
    );
    scratch.put(&format!("{id}/LICENSE"), "agpl");
    scratch.put(&format!("{id}/THIRD-PARTY-LICENSES.txt"), "n");
    let file = scratch.put(
        &format!("{id}/bin/{program}{}", std::env::consts::EXE_SUFFIX),
        "p",
    );
    executable(&file).unwrap();
    scratch.0.join(id)
}

#[test]
fn a_head_part_needs_its_manifest_version_licenses_and_program() {
    let scratch = Scratch::new("head");
    let tui = head(&scratch, "tui", "miyu-tui", "0.1.0");
    assert_eq!(head_contents(&tui, "tui", "0.1.0"), Vec::<String>::new());
    assert!(head_contents(&tui, "tui", "0.2.0")[0].contains("版本是 0.1.0"));
    fs::remove_file(tui.join(format!("bin/miyu-tui{}", std::env::consts::EXE_SUFFIX))).unwrap();
    fs::remove_file(tui.join("LICENSE")).unwrap();
    let found = head_contents(&tui, "tui", "0.1.0");
    assert!(
        found
            .iter()
            .any(|one| one.contains("少了程序 bin/miyu-tui")),
        "{found:?}"
    );
    assert!(
        found.iter().any(|one| one.contains("少了 LICENSE")),
        "{found:?}"
    );
    let web = head(&scratch, "webui", "miyu-web", "0.1.0");
    assert!(
        head_contents(&web, "webui", "0.1.0")
            .iter()
            .any(|one| one.contains("pages/index.html"))
    );
    scratch.put("webui/pages/index.html", "<html>");
    assert_eq!(head_contents(&web, "webui", "0.1.0"), Vec::<String>::new());
}

#[test]
fn without_the_core_part_beside_it_the_distro_check_says_so() {
    let scratch = Scratch::new("nocore");
    let found = as_distro(
        &scratch.0.join("miyu-core-0.0.0-t.tar.gz"),
        &scratch.0,
        &scratch.0,
        "tui",
    )
    .unwrap();
    assert!(found[0].contains("旁边没有核心那一份"), "{found:?}");
}
