//! 找技能（施工 X-10 上）：装了的包里的、自己放的；同名的自己放的赢，两个包撞名留编号在前的；没装的包、读坏了的清单不算；
//! 开头写错的跳过、说为什么；点开头的文件夹、没有 `SKILL.md` 的文件夹不算。

use std::fs;

use super::*;
use crate::env::{Env, Platform};
use crate::packages::Packages;
use crate::resources::ResourceRoot;
use crate::test_support::{Scratch, link};

struct Places {
    scratch: Scratch,
    root: DataRoot,
    resources: ResourceRoot,
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
            scratch,
        }
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.scratch.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn found(&self) -> (Vec<Skill>, Vec<Skipped>) {
        self.found_with(&[])
    }

    /// 同 [`Places::found`]，另读临时目录里的这几个别的目录（施工 PT-2 外），照先后。
    fn found_with(&self, external: &[&str]) -> (Vec<Skill>, Vec<Skipped>) {
        let packages = Packages::new(&self.resources, &self.root).read();
        let external: Vec<External> = external
            .iter()
            .map(|text| External {
                text: (*text).to_string(),
                dir: self.scratch.path().join(text),
            })
            .collect();
        find(&packages, &self.root, &external)
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
    places.write("data/skills/shared/SKILL.md", &skill("shared", "My own."));
    places.write("data/skills/notes/SKILL.md", &skill("notes", "Mine."));
    places.write("data/skills/.hidden/SKILL.md", &skill(".hidden", "No."));
    places.write("data/skills/empty/README.md", "没有 SKILL.md");
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
    assert!(notes.file.ends_with("skills/notes/SKILL.md"));
    assert!(notes.dir.ends_with("skills/notes"));
    assert_eq!(skipped.len(), 1, "撞名的那一个：{skipped:?}");
    assert!(
        skipped[0]
            .path
            .ends_with("res/packages/tools/skills/shared")
    );
    assert!(skipped[0].why.contains("already provided by "));
    // 被谁盖住（施工 X-13 中）：自己放的那一份 `SKILL.md`。
    let by = skipped[0].by.as_ref().expect("同名的记了被谁盖住");
    assert!(by.ends_with("data/skills/shared/SKILL.md"), "{by:?}");
}

/// 两个包撞了名，出厂的先于家目录（施工 F-8 六补，设计 31 定了的 J）：核心升级后出厂的新带了同名的，编号排在前的家目录
/// 的包也让给它；同一层照编号。
#[test]
fn a_shipped_package_wins_over_a_home_one_whatever_the_ids() {
    let places = Places::new();
    places.write("res/packages/z-tools/package.toml", &package("z-tools"));
    places.write(
        "res/packages/z-tools/skills/pdf/SKILL.md",
        &skill("pdf", "Shipped."),
    );
    places.write("data/packages/a-kit/package.toml", &package("a-kit"));
    places.write(
        "data/packages/a-kit/skills/pdf/SKILL.md",
        &skill("pdf", "Installed."),
    );
    let (skills, skipped) = places.found();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].package.as_deref(), Some("z-tools"));
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    assert!(skipped[0].path.ends_with("packages/a-kit/skills/pdf"));
}

#[test]
fn a_wrong_head_is_skipped_and_said_why() {
    let places = Places::new();
    // 没有说明的读不成（施工 PT-2 上：名字和文件夹不同名的照收，见下）。
    places.write("data/skills/bad/SKILL.md", "---\nname: bad\n---\n正文\n");
    places.write("data/skills/good/SKILL.md", &skill("good", "Fine."));
    // 名字和文件夹不同名的照收、照写的名字用、带一条警告（施工 PT-2 上）。
    places.write(
        "data/skills/zip-main/SKILL.md",
        &skill("zip", "Unpacked from a zip."),
    );
    let (skills, skipped) = places.found();
    assert_eq!(
        skills
            .iter()
            .map(|skill| (skill.head.name.as_str(), skill.head.warnings.len()))
            .collect::<Vec<_>>(),
        [("good", 0), ("zip", 1)]
    );
    assert!(skills[1].dir.ends_with("skills/zip-main"));
    assert_eq!(skipped.len(), 1);
    assert!(skipped[0].path.ends_with("skills/bad/SKILL.md"));
    assert_eq!(skipped[0].why, "description is missing or empty");
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
    places.write("kit/skills/bad/SKILL.md", "---\nname: bad\n---\n正文\n");
    places.write("kit/skills/empty/README.md", "nothing");
    let dir = places.scratch.path().join("kit");
    let (names, skipped) = carried(&dir);
    assert_eq!(names, ["alpha", "notes"]);
    // 读不成的另交回为什么（施工 X-13 补）。
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    assert!(skipped[0].path.ends_with("kit/skills/bad/SKILL.md"));
    assert_eq!(skipped[0].why, "description is missing or empty");
    assert_eq!(
        carried(&places.scratch.path().join("nothing")),
        (Vec::new(), Vec::new())
    );
}

/// 技能文件夹是链接（施工 X-37）：照它指到的真实位置记，链接本身另记（删的时候只删它）；指到数据根里别处的跳过、带真实位置；
/// 开头写错的照它在 `skills/` 里的位置报。
#[test]
fn a_linked_skill_folder_is_found_where_it_really_is() {
    let places = Places::new();
    places.write("elsewhere/notes/SKILL.md", &skill("notes", "Take notes."));
    places.write("data/keys/SKILL.md", &skill("keys", "Not mine."));
    places.write("elsewhere/bad/SKILL.md", "---\nname: bad\n---\n正文\n");
    let skills_dir = places.root.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();
    let (notes, keys) = (skills_dir.join("notes"), skills_dir.join("keys"));
    if !(link(&places.scratch.path().join("elsewhere/notes"), &notes)
        && link(&places.scratch.path().join("data/keys"), &keys)
        && link(
            &places.scratch.path().join("elsewhere/bad"),
            &skills_dir.join("bad"),
        ))
    {
        return;
    }
    let (skills, skipped) = places.found();
    let real = fs::canonicalize(places.scratch.path().join("elsewhere/notes")).unwrap();
    assert_eq!(
        skills
            .iter()
            .map(|skill| (skill.dir.clone(), skill.file.clone(), skill.link.clone()))
            .collect::<Vec<_>>(),
        [(real.clone(), real.join("SKILL.md"), Some(notes))]
    );
    assert_eq!(skipped.len(), 2, "{skipped:?}");
    assert_eq!(
        (skipped[0].path.clone(), skipped[0].outside.clone()),
        (
            keys,
            fs::canonicalize(places.scratch.path().join("data/keys")).ok()
        )
    );
    assert_eq!(skipped[1].path, skills_dir.join("bad").join("SKILL.md"));
}

/// 别的目录（施工 PT-2 中、PT-2 外）：排在 Miyu 自己的（自己放的、包里的）后面，几个之间照先后；同名的先到的赢，别的目录里
/// 被压掉的不记；标着配置里写的那个目录。没交的不碰，没有的目录不算。
#[test]
fn external_folders_come_after_miyu_s_own_in_their_order() {
    let places = Places::new();
    places.write("res/packages/tools/package.toml", &package("tools"));
    places.write(
        "res/packages/tools/skills/pdf/SKILL.md",
        &skill("pdf", "From the package."),
    );
    places.write("data/skills/mine/SKILL.md", &skill("mine", "My own."));
    places.write("first/mine/SKILL.md", &skill("mine", "First."));
    places.write("first/pdf/SKILL.md", &skill("pdf", "First."));
    places.write("first/notes/SKILL.md", &skill("notes", "First."));
    places.write("second/notes/SKILL.md", &skill("notes", "Second."));
    places.write("second/todo/SKILL.md", &skill("todo", "Second."));
    let (skills, skipped) = places.found_with(&["missing", "first", "second"]);
    let brief: Vec<(&str, &str, Option<&str>)> = skills
        .iter()
        .map(|found| {
            (
                found.head.name.as_str(),
                found.head.description.as_str(),
                found.external.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        brief,
        [
            ("mine", "My own.", None),
            ("notes", "First.", Some("first")),
            ("pdf", "From the package.", None),
            ("todo", "Second.", Some("second")),
        ]
    );
    assert!(skills[1].file.ends_with("first/notes/SKILL.md"));
    assert_eq!(skills[1].package, None);
    assert!(skipped.is_empty(), "别的目录里被压掉的不记：{skipped:?}");

    let names: Vec<String> = places
        .found()
        .0
        .into_iter()
        .map(|one| one.head.name)
        .collect();
    assert_eq!(names, ["mine", "pdf"], "没交的不碰");
}
