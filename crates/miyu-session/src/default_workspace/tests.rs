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
    let main = SessionId::parse("0199a1b2-0000-7000-8000-00000000abcd").expect("合写法");
    let place = DefaultWorkspace::new(&root, &main);
    let data = base.join("data");
    let fallback = data.join("workspace").join(main.as_str());
    let root_dir = PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" });
    assert_eq!(
        place.of(&root_dir, Some(&base), &data),
        fallback,
        "根目录太宽"
    );
    assert_eq!(
        place.of(&data.join("state"), Some(&base), &data),
        fallback,
        "落在数据根里太宽"
    );
    // 家目录（这里还包住数据根）：施工 5-14 改起不太宽，Windows 施工 SW-7 跟上。
    assert_eq!(place.of(&base, Some(&base), &data), base, "家目录");
    let project = base.join("proj");
    assert_eq!(
        place.of(&project, Some(&base), &data),
        project,
        "项目目录就是它自己"
    );
}
