use super::*;

use miyu_store::env::{Env, Platform};

/// 一个不存在的数据根：只算路径，不碰磁盘。
fn root_at(base: &Path) -> DataRoot {
    let env = Env {
        platform: Platform::current(),
        miyu_home: Some(base.join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    DataRoot::locate(&env).expect("MIYU_HOME 是绝对路径")
}

#[test]
fn a_too_wide_directory_works_in_the_session_s_own_folder() {
    let base = std::env::temp_dir().join("miyu-workplace-never-made");
    let root = root_at(&base);
    let owner = AccountId::parse("alice").expect("合写法");
    let main = SessionId::parse("0199a1b2-0000-7000-8000-00000000abcd").expect("合写法");
    let place = DefaultWorkspace::new(&root, &owner, &main);
    let data = base.join("data");
    let fallback = data
        .join("home")
        .join("alice")
        .join("workspace")
        .join(main.as_str());
    assert_eq!(place.of(&base, Some(&base), &data), fallback, "家目录太宽");
    let project = base.join("proj");
    assert_eq!(
        place.of(&project, Some(&base), &data),
        project,
        "项目目录就是它自己"
    );
}
