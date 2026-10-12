//! 找脚本（施工 X-12 上）：装了的包里的、自己放的；同一个工具名的自己放的赢；读坏了的清单不算；开头写错的、不是字的跳过、
//! 说为什么；子文件夹、点开头的不算。

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

    fn write(&self, path: &str, bytes: &[u8]) {
        let path = self.scratch.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn found(&self) -> (Vec<Script>, Vec<Skipped>) {
        let packages = Packages::new(&self.resources, &self.root, &self.admin).read();
        find(&packages, &self.root, &self.admin)
    }
}

fn script(description: &str) -> Vec<u8> {
    format!("#!/bin/sh\n# Description: {description}\necho hi\n").into_bytes()
}

#[test]
fn scripts_come_from_installed_packages_and_the_home_folder() {
    let places = Places::new();
    places.write(
        "res/packages/tools/package.toml",
        b"[package]\nprotocol = [1, 1]\nname = { en = \"tools\" }\n\n[builtin]\n",
    );
    places.write(
        "res/packages/tools/scripts/weather.py",
        &script("From the package."),
    );
    places.write(
        "res/packages/tools/scripts/shared.sh",
        &script("From the package."),
    );
    places.write("data/home/admin/scripts/shared", &script("My own."));
    places.write("data/home/admin/scripts/notes", &script("Mine."));
    places.write("data/home/admin/scripts/.hidden", &script("No."));
    places.write("data/home/admin/scripts/nested/inner", &script("No."));
    places.write(
        "data/home/admin/scripts/broken",
        b"#!/bin/sh\necho no header\n",
    );
    places.write("data/home/admin/scripts/binary", &[0xff, 0xfe, 0x00]);
    places.write("res/packages/bad/package.toml", b"[package\n");
    places.write("res/packages/bad/scripts/lost", &script("No."));
    let (scripts, skipped) = places.found();
    let brief: Vec<(&str, &str, Option<&str>)> = scripts
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
            ("notes", "Mine.", None),
            ("shared", "My own.", None),
            ("weather", "From the package.", Some("tools")),
        ],
        "照工具名排，同名的自己放的赢"
    );
    assert!(
        scripts[2]
            .file
            .ends_with("res/packages/tools/scripts/weather.py")
    );
    let why: Vec<(String, &str)> = skipped
        .iter()
        .map(|one| {
            (
                one.path.file_name().unwrap().to_string_lossy().into_owned(),
                one.why.as_str(),
            )
        })
        .collect();
    assert_eq!(
        why,
        [
            ("binary".to_string(), "not text"),
            ("broken".to_string(), "Description is missing or empty"),
            (
                "shared.sh".to_string(),
                "script \"shared\" is already provided elsewhere"
            ),
        ]
    );
}

#[test]
fn nothing_anywhere_is_empty() {
    assert_eq!(Places::new().found(), (Vec::new(), Vec::new()));
}

/// 装之前给人看的（施工 X-13 上）：包文件夹 `scripts/` 下开头读得成的工具名，照文件名排；写错的、子文件夹、点开头的不算。
#[test]
fn a_package_folder_says_which_scripts_it_carries() {
    let places = Places::new();
    places.write("kit/scripts/weather.py", &script("Weather."));
    places.write("kit/scripts/b-dice.sh", &script("Dice."));
    places.write("kit/scripts/broken.sh", b"#!/bin/sh\necho hi\n");
    places.write("kit/scripts/.hidden.sh", &script("Hidden."));
    places.write("kit/scripts/sub/inner.sh", &script("Inner."));
    let dir = places.scratch.path().join("kit");
    assert_eq!(carried(&dir), ["b-dice", "weather"]);
    assert!(carried(&places.scratch.path().join("nothing")).is_empty());
}
