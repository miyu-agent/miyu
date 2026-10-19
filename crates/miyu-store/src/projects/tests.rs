//! 项目的文件（施工 H-1）：没有的是空的；写了读回来一样，没写的格不写进文件；人格三种状态（没写、`null`、编号）分得开；各格的
//! 写法；改过的不写；版本不对的说是坏了。

use super::*;
use crate::test_support::Scratch;

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("合写法")
}

fn project(id: &str, name: &str) -> Project {
    Project {
        project: id.to_string(),
        name: name.to_string(),
        description: None,
        color: None,
        workspace: None,
        persona: None,
        preset: None,
        prompt: None,
        open: false,
        created: at("2026-10-19T00:00:00.000Z"),
    }
}

#[test]
fn missing_is_empty_and_a_written_one_reads_back() {
    let scratch = Scratch::new();
    let path = scratch.path().join(FILE);
    let empty = read(&path).expect("没有的是空的");
    assert_eq!(empty.projects, Projects::default());
    assert_eq!(empty.version, None);

    let mut projects = Projects::default();
    let mut full = project("p1", "Miyu");
    full.description = Some("重制".to_string());
    full.color = Some("#a1B2c3".to_string());
    full.workspace = Some("~/src/miyu".to_string());
    full.persona = Some(None);
    full.preset = Some("full".to_string());
    full.prompt = Some("Answer in Chinese.".to_string());
    full.open = true;
    projects.projects.push(full);
    let mut named = project("p2", "Notes");
    named.persona = Some(Some("engineer".to_string()));
    projects.projects.push(named);
    projects.projects.push(project("p3", "Bare"));
    write(&path, &projects, None).expect("写得进");

    let back = read(&path).expect("读得回");
    assert_eq!(back.projects, projects);
    let text = std::fs::read_to_string(&path).expect("在");
    assert!(text.contains(r#""persona":null"#), "无人格写 null：{text}");
    assert!(text.contains(r#""persona":"engineer""#), "{text}");
    let bare = r#"{"project":"p3","name":"Bare","created":"#;
    assert!(text.contains(bare), "没写的格不写：{text}");
    assert!(!text.contains(r#""open":false"#), "{text}");
}

#[test]
fn a_changed_file_is_not_overwritten_and_a_wrong_version_is_bad() {
    let scratch = Scratch::new();
    let path = scratch.path().join(FILE);
    let mut projects = Projects::default();
    projects.projects.push(project("p1", "A"));
    write(&path, &projects, None).expect("写得进");
    let first = read(&path).expect("读得回");
    projects.projects.push(project("p2", "B"));
    write(&path, &projects, first.version.as_deref()).expect("照读到的版本写");
    assert!(
        matches!(
            write(&path, &projects, first.version.as_deref()),
            Err(WriteError::Changed)
        ),
        "版本对不上不写"
    );
    std::fs::write(&path, "{\"version\":2,\"projects\":[]}\n").expect("写得进");
    assert!(matches!(read(&path), Err(BadFile::Shape(_))));
}

#[test]
fn each_field_is_checked() {
    let fine = project("p1", "Miyu");
    assert_eq!(fine.check(), Ok(()));
    let with = |change: &dyn Fn(&mut Project)| {
        let mut one = fine.clone();
        change(&mut one);
        one.check()
    };
    assert_eq!(with(&|p| p.name = String::new()), Err(Wrong::Name));
    assert_eq!(
        with(&|p| p.name = " Miyu".to_string()),
        Err(Wrong::Name),
        "首尾不留空白"
    );
    assert_eq!(with(&|p| p.name = "字".repeat(40)), Ok(()));
    assert_eq!(with(&|p| p.name = "字".repeat(41)), Err(Wrong::Name));
    assert_eq!(with(&|p| p.description = Some("字".repeat(80))), Ok(()));
    assert_eq!(
        with(&|p| p.description = Some("字".repeat(81))),
        Err(Wrong::Description)
    );
    assert_eq!(with(&|p| p.color = Some("#00ff7F".to_string())), Ok(()));
    for color in ["00ff7f", "#00ff7", "#00ff7g", "#00ff7f0", "red"] {
        assert_eq!(
            with(&|p| p.color = Some(color.to_string())),
            Err(Wrong::Color),
            "{color}"
        );
    }
    assert_eq!(
        with(&|p| p.workspace = Some(" ".to_string())),
        Err(Wrong::Workspace)
    );
    assert_eq!(with(&|p| p.prompt = Some("字".repeat(8000))), Ok(()));
    assert_eq!(
        with(&|p| p.prompt = Some("字".repeat(8001))),
        Err(Wrong::Prompt)
    );
    assert_eq!(Wrong::Prompt.field(), "prompt");
}

#[test]
fn find_and_remove_by_id() {
    let mut projects = Projects::default();
    projects.projects.push(project("p1", "A"));
    projects.projects.push(project("p2", "B"));
    assert_eq!(projects.find("p2").map(|p| p.name.as_str()), Some("B"));
    if let Some(found) = projects.find_mut("p1") {
        found.open = true;
    }
    assert_eq!(projects.find("p1").map(|p| p.open), Some(true));
    assert_eq!(projects.remove("p1").map(|p| p.name), Some("A".to_string()));
    assert_eq!(projects.remove("p1"), None);
    assert_eq!(projects.projects.len(), 1);
}
