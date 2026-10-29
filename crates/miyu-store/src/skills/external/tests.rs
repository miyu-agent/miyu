//! 读别的目录里的技能的配置（施工 PT-2 外）：关着的一个都不交；开着的照表里的先后，`~` 照家目录拼，写法不认的、没有家目录的
//! `~` 跳过，重了的留前一个。

use super::*;

fn values(pairs: &[(&str, Value)]) -> Values {
    let mut values = Values::default();
    for (key, value) in pairs {
        values.set(key, value.clone());
    }
    values
}

fn texts(texts: &[&str]) -> Value {
    Value::List(
        texts
            .iter()
            .map(|text| Value::Text(Cow::Owned(text.to_string())))
            .collect::<Vec<_>>()
            .into(),
    )
}

#[test]
fn off_or_unwritten_reads_nothing() {
    let home = Path::new("/home/me");
    assert!(of(&values(&[]), Some(home)).is_empty(), "出厂关");
    let off = values(&[
        (ENABLED, Value::Bool(false)),
        (DIRS, texts(&["/srv/skills"])),
    ]);
    assert!(of(&off, Some(home)).is_empty());
}

#[test]
fn on_reads_the_listed_folders_in_order() {
    let home = std::env::temp_dir().join("me");
    let shipped = of(&values(&[(ENABLED, Value::Bool(true))]), Some(&home));
    assert_eq!(
        shipped,
        [External {
            text: SHIPPED.to_string(),
            dir: home.join(".agents").join("skills"),
        }],
        "没写目录表的照出厂那一项"
    );
    let absolute = std::env::temp_dir().join("shared");
    let absolute_text = absolute.to_string_lossy().into_owned();
    let listed = values(&[
        (ENABLED, Value::Bool(true)),
        (
            DIRS,
            texts(&["~/b", "relative/c", &absolute_text, "~/b/", "~"]),
        ),
    ]);
    let found: Vec<(String, PathBuf)> = of(&listed, Some(&home))
        .into_iter()
        .map(|one| (one.text, one.dir))
        .collect();
    assert_eq!(
        found,
        [
            ("~/b".to_string(), home.join("b")),
            (absolute_text.clone(), absolute.clone()),
            ("~".to_string(), home.clone()),
        ],
        "相对路径不认，重了的留前一个"
    );
    let homeless: Vec<String> = of(&listed, None).into_iter().map(|one| one.text).collect();
    assert_eq!(homeless, [absolute_text], "没有家目录的 ~ 跳过");
    let empty = values(&[(ENABLED, Value::Bool(true)), (DIRS, texts(&[]))]);
    assert!(of(&empty, Some(&home)).is_empty(), "写了空表的就是空的");
}

#[test]
fn only_home_and_absolute_paths_are_well_written() {
    assert!(well_written("~"));
    assert!(well_written("~/.agents/skills"));
    assert!(well_written(&std::env::temp_dir().to_string_lossy()));
    assert!(!well_written("skills"));
    assert!(!well_written("./skills"));
    assert!(!well_written("~other/skills"));
    assert!(!well_written(""));
}
