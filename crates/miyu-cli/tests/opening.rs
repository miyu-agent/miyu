//! `miyu ask` 新开的会话怎么开（施工 AP-2，设计 50 第三节）：在进程里起一个核心，在真的套接字上走一遍。`--persona`、
//! `--no-persona`、`--preset` 跟着 `session.create` 走，记进 `session.created`；`--permission` 在开好以后、发话之前切，日志里
//! 权限换了那一条在人说的话前面；没有的人格照核心的原话说，退出码 1，不发话。

use std::sync::Arc;

use crate::support::{Asked, Home, plan};
use miyu_cli::{Opening, Permission, PersonaChoice, Plan};
use miyu_kernel::event::{Body, Level, SessionCreated};
use miyu_session::testkit::{Play, Script};
use miyu_store::log::first_event;

/// 照 `opening` 开一个新会话说一句，交回它的 `session.created`。
async fn created(home: &Home, opening: Opening) -> SessionCreated {
    let asking = Plan {
        opening,
        ..plan("在吗")
    };
    let Asked { code, err, .. } = home.ask(&asking).await;
    assert_eq!(code, 0, "{err}");
    let session = home.sessions()[0].clone();
    match first_event(&home.root.session_dir(&session))
        .expect("读得到")
        .body
    {
        Body::SessionCreated(created) => created,
        other => panic!("第 1 条应该是造会话：{other:?}"),
    }
}

#[tokio::test]
async fn persona_and_preset_go_with_the_new_session() {
    let script = || Arc::new(Script::new([Play::Says("好。")]));
    // 配置里默认是出厂的软件工程师：`--no-persona` 不用它。
    let config = "[persona]\ndefault = \"engineer\"\n";
    let home = Home::configured(script(), config);
    let without = Opening {
        persona: PersonaChoice::Without,
        ..Opening::default()
    };
    assert_eq!(created(&home, without).await.persona, None);
    let home = Home::configured(script(), config);
    assert_eq!(
        created(&home, Opening::default()).await.persona.as_deref(),
        Some("engineer"),
        "不写的照配置"
    );
    let home = Home::new(script());
    let named = Opening {
        persona: PersonaChoice::Named("engineer".to_string()),
        preset: Some("minimal".to_string()),
        ..Opening::default()
    };
    let made = created(&home, named).await;
    assert_eq!(made.persona.as_deref(), Some("engineer"));
    assert_eq!(made.preset.as_deref(), Some("minimal"));
}

#[tokio::test]
async fn an_unknown_persona_is_refused_and_nothing_is_said() {
    let home = Home::new(Arc::new(Script::new([])));
    let asking = Plan {
        opening: Opening {
            persona: PersonaChoice::Named("no-such-persona".to_string()),
            ..Opening::default()
        },
        ..plan("在吗")
    };
    let Asked { code, out, err, .. } = home.ask(&asking).await;
    assert_eq!(code, 1, "{err}");
    assert_eq!(out, "");
    assert!(err.starts_with("miyu: "), "{err}");
    assert!(home.sessions().is_empty(), "没造会话");
}

/// `--permission`：开好以后、说话之前切到那一级，日志里换了的那一条在人说的话前面。`read-only` 只开只读开关，另两种关上只读、
/// 换常用的那一级。
#[tokio::test]
async fn permission_is_set_before_the_first_words() {
    for (permission, level, read_only) in [
        (Permission::ReadOnly, Level::Workspace, true),
        (Permission::Workspace, Level::Workspace, false),
        (Permission::Full, Level::Full, false),
    ] {
        // `workspace`、`full` 在配置里开局只读的会话上切：明写的放宽，关上只读；`read-only` 在不只读的会话上切。
        let config = match permission {
            Permission::ReadOnly => "",
            _ => "[permission]\nstart_read_only = true\n",
        };
        let home = Home::configured(Arc::new(Script::new([Play::Says("好。")])), config);
        let asking = Plan {
            opening: Opening {
                permission: Some(permission),
                ..Opening::default()
            },
            ..plan("在吗")
        };
        let Asked { code, err, .. } = home.ask(&asking).await;
        assert_eq!(code, 0, "{permission:?}：{err}");
        let session = home.sessions()[0].clone();
        let order: Vec<String> = home
            .log(&session)
            .into_iter()
            .filter_map(|event| match event.body {
                Body::PolicyChanged(changed) => changed
                    .permission
                    .map(|now| format!("{}/{}", now.level.as_str(), now.read_only)),
                Body::MessageUser(_) => Some("说".to_string()),
                _ => None,
            })
            .collect();
        let expected = format!("{}/{read_only}", level.as_str());
        assert_eq!(order.first(), Some(&expected), "{permission:?}：{order:?}");
        assert!(
            order.contains(&"说".to_string()),
            "{permission:?}：{order:?}"
        );
    }
}
