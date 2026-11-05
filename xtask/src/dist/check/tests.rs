//! 查包里不跑程序的那几步：解开、只有一层、照仓库比内容。

use super::*;
use crate::dist::archive::pack;
use crate::dist::layout::executable;

/// 测试的临时目录，用完删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("xtask-check-{name}-{}", std::process::id()));
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

const TARGET: &str = "linux-x86_64";

/// 一个仓库（`repo/`）和照它搭好的一个包（`pkg/`），交回仓库里登记了的资源。
fn good(scratch: &Scratch) -> Vec<PathBuf> {
    scratch.put("repo/LICENSE", "agpl");
    scratch.put("repo/THIRD-PARTY-LICENSES.txt", "core notices\n\n");
    scratch.put("repo/resources/core/a.md", "a");
    scratch.put("repo/resources/packages/miyu-base-x/package.toml", "x");
    for program in [
        "miyu",
        "miyu-sandbox",
        "miyu-extract",
        "miyu-tui",
        "miyu-web",
    ] {
        let file = scratch.put(&format!("pkg/{program}"), "program");
        executable(&file).unwrap();
    }
    scratch.put("pkg/LICENSE", "agpl");
    scratch.put(
        "pkg/THIRD-PARTY-LICENSES.txt",
        "header\n\ncore notices\n\ntui\nweb\n",
    );
    scratch.put("pkg/README.txt", "Miyu 1.0.0 (linux-x86_64)\n\n中文\n");
    scratch.put("pkg/resources/core/a.md", "a");
    scratch.put("pkg/resources/packages/miyu-base-x/package.toml", "x");
    for file in [
        "miyu-tui/package.toml",
        "miyu-tui/LICENSE",
        "miyu-tui/THIRD-PARTY-LICENSES.txt",
        "miyu-webui/package.toml",
        "miyu-webui/LICENSE",
        "miyu-webui/THIRD-PARTY-LICENSES.txt",
        "miyu-webui/pages/index.html",
        "miyu-webui/pages/src/app.js",
    ] {
        scratch.put(&format!("pkg/resources/packages/{file}"), "head");
    }
    vec![
        PathBuf::from("resources/core/a.md"),
        PathBuf::from("resources/packages/miyu-base-x/package.toml"),
    ]
}

fn problems(scratch: &Scratch, resources: &[PathBuf]) -> Vec<String> {
    contents(
        &scratch.0.join("repo"),
        &scratch.0.join("pkg"),
        "1.0.0",
        TARGET,
        resources,
    )
    .unwrap()
}

#[test]
fn a_package_built_from_the_repository_passes() {
    let scratch = Scratch::new("good");
    let resources = good(&scratch);
    assert_eq!(problems(&scratch, &resources), Vec::<String>::new());
}

/// 一种弄坏法：名字、怎么弄坏、报出来的话里该有什么。
type Case = (&'static str, fn(&Scratch), &'static str);

#[test]
fn every_mistake_is_named() {
    let cases: [Case; 10] = [
        (
            "program",
            |s| fs::remove_file(s.0.join("pkg/miyu-web")).unwrap(),
            "少了程序 miyu-web",
        ),
        (
            "license",
            |s| drop(s.put("pkg/LICENSE", "mit")),
            "LICENSE 和仓库的不一样",
        ),
        (
            "notices",
            |s| drop(s.put("pkg/THIRD-PARTY-LICENSES.txt", "tui\n")),
            "没有核心那一份",
        ),
        (
            "readme",
            |s| drop(s.put("pkg/README.txt", "Miyu 0.9.0 (linux-x86_64)\n")),
            "README.txt 开头",
        ),
        (
            "missing",
            |s| fs::remove_file(s.0.join("pkg/resources/core/a.md")).unwrap(),
            "少了资源 resources/core/a.md",
        ),
        (
            "changed",
            |s| drop(s.put("pkg/resources/core/a.md", "b")),
            "和仓库的不一样",
        ),
        (
            "extra",
            |s| drop(s.put("pkg/resources/core/stray.md", "?")),
            "resources/core/stray.md 不在仓库的清单里",
        ),
        (
            "tui",
            |s| fs::remove_file(s.0.join("pkg/resources/packages/miyu-tui/package.toml")).unwrap(),
            "少了 resources/packages/miyu-tui/package.toml",
        ),
        (
            "web",
            |s| {
                fs::remove_file(s.0.join("pkg/resources/packages/miyu-webui/pages/index.html"))
                    .unwrap()
            },
            "pages/index.html",
        ),
        (
            "bin",
            |s| drop(s.put("pkg/resources/packages/miyu-webui/bin/miyu-web", "x")),
            "bin/ 不该有",
        ),
    ];
    for (name, spoil, said) in cases {
        let scratch = Scratch::new(name);
        let resources = good(&scratch);
        spoil(&scratch);
        let found = problems(&scratch, &resources);
        assert!(
            found.iter().any(|one| one.contains(said)),
            "{name}：{found:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_program_without_the_bit_is_named() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("exec");
    let resources = good(&scratch);
    fs::set_permissions(
        scratch.0.join("pkg/miyu"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    let found = problems(&scratch, &resources);
    assert!(
        found.iter().any(|one| one.contains("miyu 不能跑")),
        "{found:?}"
    );
}

#[test]
fn unpacking_gives_the_one_folder_for_both_kinds() {
    for kind in ["tar.gz", "zip"] {
        let scratch = Scratch::new(&format!("unpack-{}", kind.replace('.', "")));
        scratch.put("miyu-1.0.0-t/resources/b.txt", "bee");
        let program = scratch.put("miyu-1.0.0-t/miyu", "program");
        executable(&program).unwrap();
        let archive = scratch.0.join(format!("a.{kind}"));
        pack(&scratch.0.join("miyu-1.0.0-t"), "miyu-1.0.0-t", &archive).unwrap();
        let into = scratch.0.join("out");
        fs::create_dir_all(&into).unwrap();
        unpack(&archive, &into).unwrap();
        assert_eq!(top(&into, "miyu-1.0.0-t").unwrap(), Vec::<String>::new());
        assert_eq!(
            fs::read_to_string(into.join("miyu-1.0.0-t/resources/b.txt")).unwrap(),
            "bee"
        );
        assert!(
            runnable(&into.join("miyu-1.0.0-t/miyu")),
            "{kind} 解开后能跑"
        );
        let said = top(&into, "miyu-2.0.0-t").unwrap();
        assert!(said[0].contains("只有一层 miyu-2.0.0-t/"), "{said:?}");
    }
}

#[test]
fn a_broken_archive_is_an_error() {
    let scratch = Scratch::new("broken");
    let archive = scratch.put("a.tar.gz", "not gzip");
    let into = scratch.0.join("out");
    fs::create_dir_all(&into).unwrap();
    assert!(unpack(&archive, &into).unwrap_err().contains("解不开"));
}

#[test]
fn relative_paths_are_slashed_whichever_way_they_were_written() {
    assert_eq!(relative(Path::new("resources/core/a.md")), "core/a.md");
    assert_eq!(relative(&Path::new("core").join("a.md")), "core/a.md");
    assert_eq!(relative(Path::new("resources\\core\\a.md")), "core/a.md");
    assert_eq!(relative(Path::new("core\\a.md")), "core/a.md");
}

#[test]
fn a_listed_resource_matches_the_walked_file_on_every_system() {
    let scratch = Scratch::new("slash");
    let resources = good(&scratch);
    // 清单里写成 Windows 的样子也认：比的时候一律 `/`。
    let windows: Vec<PathBuf> = resources
        .iter()
        .map(|file| PathBuf::from(file.to_string_lossy().replace('/', "\\")))
        .collect();
    assert_eq!(problems(&scratch, &windows), Vec::<String>::new());
}
