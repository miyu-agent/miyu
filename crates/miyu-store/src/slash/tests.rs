//! 找斜杠命令模板（施工 X-14 上）：装了的包里的、自己放的；同名的自己放的赢，包之间出厂的先；读坏了的清单不算；名字不合
//! 写法、开头写错的跳过、说为什么；子文件夹、点开头的、不是 `.md` 的不算。

use std::fs;

use super::*;
use crate::env::{Env, Platform};
use crate::packages::Packages;
use crate::resources::ResourceRoot;
use crate::test_support::Scratch;

struct Places {
    scratch: Scratch,
    root: DataRoot,
    resources: ResourceRoot,
    admin: AccountId,
}

impl Places {
    fn new() -> Places {
        let scratch = Scratch::new();
        let env = Env {
            platform: Platform::current(),
            miyu_home: Some(scratch.path().join("data").into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        };
        Places {
            resources: ResourceRoot::at(scratch.path().join("res")),
            root: DataRoot::locate(&env).unwrap(),
            admin: AccountId::parse("admin").unwrap(),
            scratch,
        }
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.scratch.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn found(&self) -> (Vec<Command>, Vec<Skipped>) {
        let packages = Packages::new(&self.resources, &self.root, &self.admin).read();
        find(&packages, &self.root, &self.admin)
    }
}

fn package(id: &str) -> String {
    format!("[package]\nname = {{ en = \"{id}\" }}\n")
}

fn brief(commands: &[Command]) -> Vec<(&str, &str, Option<&str>)> {
    commands
        .iter()
        .map(|found| {
            (
                found.name.as_str(),
                found.template.body.as_str(),
                found.package.as_deref(),
            )
        })
        .collect()
}

#[test]
fn commands_come_from_installed_packages_and_the_home_folder() {
    let places = Places::new();
    places.write("res/packages/tools/package.toml", &package("tools"));
    places.write(
        "res/packages/tools/commands/review.md",
        "From the package.\n",
    );
    places.write(
        "res/packages/tools/commands/shared.md",
        "From the package.\n",
    );
    places.write("data/home/admin/commands/shared.md", "My own.\n");
    places.write(
        "data/home/admin/commands/notes.md",
        "---\ndescription: Notes\n---\nMine.\n",
    );
    places.write("data/home/admin/commands/.hidden.md", "No.\n");
    places.write("data/home/admin/commands/readme.txt", "No.\n");
    places.write("data/home/admin/commands/deeper/inner.md", "No.\n");
    // 读坏了的清单：它带的命令不算。
    places.write("res/packages/broken/package.toml", "[package\n");
    places.write("res/packages/broken/commands/lost.md", "No.\n");
    let (commands, skipped) = places.found();
    assert_eq!(
        brief(&commands),
        [
            ("notes", "Mine.\n", None),
            ("review", "From the package.\n", Some("tools")),
            ("shared", "My own.\n", None),
        ],
        "照名字排，同名的自己放的赢"
    );
    assert_eq!(commands[0].template.summary["en"], "Notes");
    assert!(commands[0].file.ends_with("home/admin/commands/notes.md"));
    assert_eq!(skipped.len(), 1, "撞名的那一个：{skipped:?}");
    assert!(
        skipped[0]
            .path
            .ends_with("res/packages/tools/commands/shared.md")
    );
    assert!(skipped[0].why.contains("already provided by "));
    let by = skipped[0].by.as_ref().expect("同名的记了被谁盖住");
    assert!(by.ends_with("data/home/admin/commands/shared.md"), "{by:?}");
}

#[test]
fn a_shipped_package_wins_over_a_home_one_whatever_the_ids() {
    let places = Places::new();
    places.write("res/packages/z-tools/package.toml", &package("z-tools"));
    places.write("res/packages/z-tools/commands/review.md", "Shipped.\n");
    places.write(
        "data/home/admin/packages/a-kit/package.toml",
        &package("a-kit"),
    );
    places.write(
        "data/home/admin/packages/a-kit/commands/review.md",
        "Installed.\n",
    );
    let (commands, skipped) = places.found();
    assert_eq!(
        brief(&commands),
        [("review", "Shipped.\n", Some("z-tools"))]
    );
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    assert!(
        skipped[0]
            .path
            .ends_with("home/admin/packages/a-kit/commands/review.md")
    );
}

#[test]
fn a_bad_name_or_head_is_skipped_and_said_why() {
    let places = Places::new();
    places.write("data/home/admin/commands/Bad_Name.md", "x\n");
    places.write(
        "data/home/admin/commands/broken.md",
        "---\ndescription: [x\n---\nbody\n",
    );
    places.write("data/home/admin/commands/fine.md", "ok\n");
    let (commands, skipped) = places.found();
    assert_eq!(brief(&commands), [("fine", "ok\n", None)]);
    let mut why: Vec<(String, &str)> = skipped
        .iter()
        .map(|one| {
            (
                one.path.file_name().unwrap().to_string_lossy().into_owned(),
                one.why.as_str(),
            )
        })
        .collect();
    why.sort();
    assert_eq!(why.len(), 2, "{why:?}");
    assert_eq!(why[0].0, "Bad_Name.md");
    assert!(
        why[0].1.contains("must start with a lowercase letter"),
        "{}",
        why[0].1
    );
    assert_eq!(why[1].0, "broken.md");
    assert!(
        why[1].1.starts_with("front matter is not valid YAML"),
        "{}",
        why[1].1
    );
}

#[test]
fn nothing_anywhere_is_empty() {
    let places = Places::new();
    let (commands, skipped) = places.found();
    assert!(commands.is_empty() && skipped.is_empty());
}

#[test]
fn a_package_folder_says_which_commands_it_carries() {
    let places = Places::new();
    places.write("kit/commands/review.md", "x\n");
    places.write("kit/commands/fix-bug.md", "x\n");
    places.write("kit/commands/Nope.md", "x\n");
    let (names, skipped) = carried(&places.scratch.path().join("kit"));
    assert_eq!(names, ["fix-bug", "review"]);
    assert_eq!(skipped.len(), 1);
}
