//! 找技能（施工 X-10 上）：装了的包里的、自己放的；同名的自己放的赢，两个包撞名留编号在前的；没装的包、读坏了的清单不算；
//! 开头写错的跳过、说为什么；点开头的文件夹、没有 `SKILL.md` 的文件夹不算。

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

    fn found(&self) -> (Vec<Skill>, Vec<Skipped>) {
        let packages = Packages::new(&self.resources, &self.root, &self.admin).read();
        find(&packages, &self.root, &self.admin)
    }
}

fn skill(name: &str, description: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\n正文\n")
}

fn package(id: &str) -> String {
    format!("[package]\nprotocol = [1, 1]\nname = {{ en = \"{id}\" }}\n\n[builtin]\n")
}

#[test]
fn skills_come_from_installed_packages_and_the_home_folder() {
    let places = Places::new();
    places.write("res/packages/tools/package.toml", &package("tools"));
    places.write(
        "res/packages/tools/skills/make-skill/SKILL.md",
        &skill("make-skill", "From the package."),
    );
    places.write(
        "res/packages/tools/skills/shared/SKILL.md",
        &skill("shared", "From the package."),
    );
    places.write(
        "data/home/admin/skills/shared/SKILL.md",
        &skill("shared", "My own."),
    );
    places.write(
        "data/home/admin/skills/notes/SKILL.md",
        &skill("notes", "Mine."),
    );
    places.write(
        "data/home/admin/skills/.hidden/SKILL.md",
        &skill(".hidden", "No."),
    );
    places.write("data/home/admin/skills/empty/README.md", "没有 SKILL.md");
    // 读坏了的清单：它带的技能不算。
    places.write("res/packages/broken/package.toml", "[package\n");
    places.write(
        "res/packages/broken/skills/lost/SKILL.md",
        &skill("lost", "No."),
    );
    let (skills, skipped) = places.found();
    let brief: Vec<(&str, &str, Option<&str>)> = skills
        .iter()
        .map(|found| {
            (
                found.head.name.as_str(),
                found.head.description.as_str(),
                found.package.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        brief,
        [
            ("make-skill", "From the package.", Some("tools")),
            ("notes", "Mine.", None),
            ("shared", "My own.", None),
        ],
        "照名字排，同名的自己放的赢"
    );
    let notes = &skills[1];
    assert!(notes.file.ends_with("home/admin/skills/notes/SKILL.md"));
    assert!(notes.dir.ends_with("home/admin/skills/notes"));
    assert_eq!(skipped.len(), 1, "撞名的那一个：{skipped:?}");
    assert!(
        skipped[0]
            .path
            .ends_with("res/packages/tools/skills/shared")
    );
    assert!(skipped[0].why.contains("already provided by "));
    // 被谁盖住（施工 X-13 中）：自己放的那一份 `SKILL.md`。
    let by = skipped[0].by.as_ref().expect("同名的记了被谁盖住");
    assert!(
        by.ends_with("data/home/admin/skills/shared/SKILL.md"),
        "{by:?}"
    );
}

#[test]
fn a_wrong_head_is_skipped_and_said_why() {
    let places = Places::new();
    places.write(
        "data/home/admin/skills/bad/SKILL.md",
        &skill("other", "Name is not the folder."),
    );
    places.write(
        "data/home/admin/skills/good/SKILL.md",
        &skill("good", "Fine."),
    );
    let (skills, skipped) = places.found();
    assert_eq!(skills.len(), 1);
    assert_eq!(skipped.len(), 1);
    assert!(skipped[0].path.ends_with("skills/bad/SKILL.md"));
    assert_eq!(
        skipped[0].why,
        "name \"other\" does not match its folder \"bad\""
    );
}

#[test]
fn nothing_anywhere_is_empty() {
    let places = Places::new();
    assert_eq!(places.found(), (Vec::new(), Vec::new()));
}

/// 装之前给人看的（施工 X-13 上）：包文件夹 `skills/` 下开头读得成的技能名，照名字排；没有 `SKILL.md` 的不算；写错的另交回
/// 为什么（施工 X-13 补）。
#[test]
fn a_package_folder_says_which_skills_it_carries() {
    let places = Places::new();
    places.write("kit/skills/notes/SKILL.md", &skill("notes", "Take notes."));
    places.write("kit/skills/alpha/SKILL.md", &skill("alpha", "First."));
    places.write("kit/skills/bad/SKILL.md", &skill("other", "Wrong name."));
    places.write("kit/skills/empty/README.md", "nothing");
    let dir = places.scratch.path().join("kit");
    let (names, skipped) = carried(&dir);
    assert_eq!(names, ["alpha", "notes"]);
    // 读不成的另交回为什么（施工 X-13 补）。
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    assert!(skipped[0].path.ends_with("kit/skills/bad/SKILL.md"));
    assert_eq!(
        skipped[0].why,
        "name \"other\" does not match its folder \"bad\""
    );
    assert_eq!(
        carried(&places.scratch.path().join("nothing")),
        (Vec::new(), Vec::new())
    );
}
