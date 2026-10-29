//! 找资源目录的几种情形，和读一个人格的原文。

use std::fs;

use super::*;
use crate::env::Platform;
use crate::test_support::Scratch;

/// 一份什么都没设的快照：`MIYU_RESOURCES` 和程序的位置照给的。
fn env(miyu_resources: Option<&Path>, exe: Option<&Path>) -> Env {
    Env {
        platform: Platform::current(),
        miyu_home: None,
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: miyu_resources.map(|path| path.as_os_str().to_os_string()),
        exe: exe.map(Path::to_path_buf),
    }
}

/// 源码树里的资源目录：出厂的那一份。
fn repo() -> ResourceRoot {
    ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

#[test]
fn miyu_resources_wins_and_must_be_an_absolute_directory() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("res");
    fs::create_dir_all(&dir).unwrap();
    // 设了的照它，程序旁边有没有都不看。
    let exe = scratch.path().join("bin").join("miyu");
    assert_eq!(
        ResourceRoot::locate(&env(Some(&dir), Some(&exe)))
            .unwrap()
            .path(),
        dir
    );
    let relative = ResourceRoot::locate(&env(Some(Path::new("resources")), None));
    assert_eq!(
        relative,
        Err(ResourceError::Relative(PathBuf::from("resources")))
    );
    let missing = scratch.path().join("nowhere");
    assert_eq!(
        ResourceRoot::locate(&env(Some(&missing), None)),
        Err(ResourceError::Missing(missing))
    );
}

#[test]
fn a_leading_tilde_is_the_home() {
    let scratch = Scratch::new();
    fs::create_dir_all(scratch.path().join("res")).unwrap();
    let tilde = |home: Option<PathBuf>| Env {
        home,
        ..env(Some(Path::new("~/res")), None)
    };
    // 开头的 `~` 照家目录接，和 `MIYU_HOME` 一样（施工 4-11）。
    let found = ResourceRoot::locate(&tilde(Some(scratch.path().to_path_buf()))).unwrap();
    assert_eq!(found.path(), scratch.path().join("res"));
    // 找不到家目录的：照原样，当相对的报错。
    assert_eq!(
        ResourceRoot::locate(&tilde(None)),
        Err(ResourceError::Relative(PathBuf::from("~/res")))
    );
}

#[test]
fn next_to_the_program_or_in_share_miyu_one_level_up() {
    // 安装脚本：~/.local/lib/miyu/miyu 旁边的 resources/。
    let scratch = Scratch::new();
    let lib = scratch.path().join("lib").join("miyu");
    fs::create_dir_all(lib.join("resources")).unwrap();
    let found = ResourceRoot::locate(&env(None, Some(&lib.join("miyu")))).unwrap();
    assert_eq!(found.path(), lib.join("resources"));
    // deb、rpm、AUR、Homebrew：<前缀>/bin/miyu 和 <前缀>/share/miyu/。
    let prefix = scratch.path().join("usr");
    fs::create_dir_all(prefix.join("bin")).unwrap();
    fs::create_dir_all(prefix.join("share").join("miyu")).unwrap();
    let found = ResourceRoot::locate(&env(None, Some(&prefix.join("bin").join("miyu")))).unwrap();
    assert_eq!(found.path(), prefix.join("share").join("miyu"));
}

/// 桌面端的 `.app`（施工 DT-4 上）：程序在 `Contents/MacOS/miyu`，资源只能放在 `Contents/Resources/` 下，
/// 照上一级下的 `Resources/miyu/` 找。照能力找：哪个平台上摆成这样都认。
#[test]
fn an_app_bundle_keeps_them_in_resources_miyu_one_level_up() {
    let scratch = Scratch::new();
    let contents = scratch.path().join("Miyu.app").join("Contents");
    fs::create_dir_all(contents.join("MacOS")).unwrap();
    fs::create_dir_all(contents.join("Resources").join("miyu")).unwrap();
    let found =
        ResourceRoot::locate(&env(None, Some(&contents.join("MacOS").join("miyu")))).unwrap();
    assert_eq!(found.path(), contents.join("Resources").join("miyu"));
    // 只有 `Resources/` 没有下面那一层的不算：`.app` 的 `Resources/` 里还放着图标这些别人的东西。
    let bare = scratch.path().join("Bare.app").join("Contents");
    fs::create_dir_all(bare.join("MacOS")).unwrap();
    fs::create_dir_all(bare.join("Resources")).unwrap();
    assert!(ResourceRoot::locate(&env(None, Some(&bare.join("MacOS").join("miyu")))).is_err());
}

/// 几处都有的，照先后：旁边的 `resources/`、上一级的 `share/miyu/`、上一级的 `Resources/miyu/`。
#[test]
fn the_places_are_tried_in_order() {
    let scratch = Scratch::new();
    let places = [
        scratch.path().join("bin").join("resources"),
        scratch.path().join("share").join("miyu"),
        scratch.path().join("Resources").join("miyu"),
    ];
    places
        .iter()
        .for_each(|place| fs::create_dir_all(place).unwrap());
    let program = scratch.path().join("bin").join("miyu");
    for place in &places {
        assert_eq!(
            ResourceRoot::locate(&env(None, Some(&program)))
                .unwrap()
                .path(),
            place
        );
        fs::remove_dir(place).unwrap();
    }
}

#[test]
fn nowhere_says_where_it_looked() {
    let scratch = Scratch::new();
    let bin = scratch.path().join("target").join("debug");
    fs::create_dir_all(&bin).unwrap();
    let error = ResourceRoot::locate(&env(None, Some(&bin.join("miyu")))).unwrap_err();
    assert_eq!(
        error,
        ResourceError::NotFound(vec![
            bin.join("resources"),
            scratch.path().join("target").join("share").join("miyu"),
            scratch.path().join("target").join("Resources").join("miyu"),
        ])
    );
    assert!(error.to_string().contains("MIYU_RESOURCES"), "{error}");
    let unknown = ResourceRoot::locate(&env(None, None)).unwrap_err();
    assert_eq!(unknown, ResourceError::NotFound(Vec::new()));
}

/// 随核心附带的字照原文读进来；人格的字照交来的（施工 P-4 下：出厂不带人格，资源目录里没有人格可读）。
#[test]
fn the_core_texts_are_read_and_the_persona_texts_are_as_given() {
    let given = PersonaTexts {
        persona: "You are a helpful software engineer.\n".to_string(),
        ..PersonaTexts::default()
    };
    let sources = repo().sources_with(given.clone()).unwrap();
    assert_eq!(sources.persona, given);
    assert!(sources.core.facts.reply_cut.starts_with("<reply-cut>"));
    // 会话编号的模板（施工 1-13 再补）。
    assert_eq!(
        sources.core.facts.session.as_deref(),
        Some(include_str!("../../../../resources/core/facts/session.txt"))
    );
    // 切了级别以后的权限那一份（施工 2-7 补）。
    assert_eq!(
        sources.core.facts.permission_changed.as_deref(),
        Some(include_str!(
            "../../../../resources/core/facts/permission-changed.txt"
        ))
    );
    assert!(
        sources
            .core
            .checkpoint_open
            .contains("conversation-checkpoint")
    );
    // 回报的写法（施工 7-2）：每一格是它自己那份文件。
    let jobs = sources.core.jobs.expect("出厂的有回报的写法");
    macro_rules! job {
        ($name:literal) => {
            include_str!(concat!("../../../../resources/core/jobs/", $name))
        };
    }
    let read = [
        (&jobs.command_open, job!("command-open.txt")),
        (&jobs.command_exit, job!("command-exit.txt")),
        (&jobs.command_signal, job!("command-signal.txt")),
        (&jobs.command_duration, job!("command-duration.txt")),
        (&jobs.command_output, job!("command-output.txt")),
        (&jobs.command_close, job!("command-close.txt")),
        (&jobs.subagent_open, job!("subagent-open.txt")),
        (&jobs.subagent_person, job!("subagent-person.txt")),
        (&jobs.subagent_truncated, job!("subagent-truncated.txt")),
        (&jobs.subagent_silent, job!("subagent-silent.txt")),
        (&jobs.subagent_close, job!("subagent-close.txt")),
        (&jobs.subagent_omitted, job!("subagent-omitted.txt")),
        (&jobs.stopped_by_user, job!("stopped-by-user.txt")),
        (
            &jobs.subagent_message_open,
            job!("subagent-message-open.txt"),
        ),
        (
            &jobs.subagent_message_close,
            job!("subagent-message-close.txt"),
        ),
    ];
    for (got, file) in read {
        assert_eq!(got, file);
    }
    // 别的 harness 发来的话的标签（施工 7-10）：每一格是它自己那份文件。
    let harness = sources
        .core
        .harness
        .expect("出厂的有别的 harness 发来的话的标签");
    assert_eq!(
        harness.message_open,
        include_str!("../../../../resources/core/harness/message-open.txt")
    );
    assert_eq!(
        harness.message_close,
        include_str!("../../../../resources/core/harness/message-close.txt")
    );
    // 别的会话发来的话的标签（施工 C-2）：每一格是它自己那份文件。
    let peers = sources.core.peers.expect("出厂的有别的会话发来的话的标签");
    assert_eq!(
        peers.message_open,
        include_str!("../../../../resources/core/peers/message-open.txt")
    );
    assert_eq!(
        peers.message_close,
        include_str!("../../../../resources/core/peers/message-close.txt")
    );
    // 回顾的五份（施工 3-8 四补）：每一格是它自己那份文件。
    let recap = sources.core.recap.expect("出厂的有回顾的字");
    macro_rules! recap {
        ($name:literal) => {
            include_str!(concat!("../../../../resources/core/recap/", $name))
        };
    }
    for (got, file) in [
        (recap.instruction, recap!("instruction.txt")),
        (recap.user, recap!("user.txt")),
        (recap.assistant, recap!("assistant.txt")),
        (recap.omitted, recap!("omitted.txt")),
        (recap.excerpted, recap!("excerpted.txt")),
    ] {
        assert_eq!(got, file);
    }
    // 起标题的指令（施工 3-8 五补）。
    assert_eq!(
        sources.core.title.expect("出厂的有起标题的字").instruction,
        include_str!("../../../../resources/core/title/instruction.txt")
    );
    // 转述一张图的两份（施工 8-17）。
    let vision = sources.core.vision.expect("出厂的有转述一张图的字");
    assert_eq!(
        vision.instruction,
        include_str!("../../../../resources/core/vision/instruction.txt")
    );
    assert_eq!(
        vision.question,
        include_str!("../../../../resources/core/vision/question.txt")
    );
    // 合并转发那几行的三句（施工 O-34）：每一格是它自己那份文件，和 `forward()` 读的一样。
    let forward = sources.core.forward.expect("出厂的有合并转发的字");
    assert_eq!(
        forward.label,
        include_str!("../../../../resources/core/venues/forward/label.txt")
    );
    assert_eq!(
        forward.numbered,
        include_str!("../../../../resources/core/venues/forward/numbered.txt")
    );
    assert_eq!(
        forward.more,
        include_str!("../../../../resources/core/venues/forward/more.txt")
    );
    assert_eq!(repo().forward().unwrap(), forward);
    // 私聊记号的语音那一句（施工 O-35）：同群会话钉下的那一份文件，和 `private_media()` 读的一样。
    let private = sources.core.private_media.expect("出厂的有私聊记号的字");
    assert_eq!(
        private.voice,
        include_str!("../../../../resources/core/venues/voice.txt")
    );
    assert_eq!(repo().private_media().unwrap(), private);
    // 文本文件照字放进消息的三句（施工 3-9 三补）：每一格是它自己那份文件。
    let drivers = sources.core.drivers;
    let text = drivers.text_file.expect("出厂的有文本文件的三句");
    macro_rules! driver {
        ($name:literal) => {
            include_str!(concat!("../../../../resources/core/drivers/", $name))
        };
    }
    assert_eq!(text.file_open, driver!("file-open.txt"));
    assert_eq!(text.file_cut, driver!("file-cut.txt"));
    assert_eq!(text.file_close, driver!("file-close.txt"));
    // 带名字的图片的三句（施工 3-9 四补）：同上。
    let image = drivers.image_name.expect("出厂的有带名字的图片的三句");
    assert_eq!(image.image_open, driver!("image-open.txt"));
    assert_eq!(image.image_close, driver!("image-close.txt"));
    assert_eq!(
        image.image_omitted_named,
        driver!("image-omitted-named.txt")
    );
    // 替它看的图的三句标签（施工 8-17）：同上。
    let wrap = drivers
        .image_description
        .expect("出厂的有替它看的图的三句标签");
    assert_eq!(
        wrap.image_description_open,
        driver!("image-description-open.txt")
    );
    assert_eq!(
        wrap.image_description_open_named,
        driver!("image-description-open-named.txt")
    );
    assert_eq!(
        wrap.image_description_close,
        driver!("image-description-close.txt")
    );
    // 看不了的附件带路径的两句（施工 3-9 五补）：同上。
    let attached = drivers
        .attached_path
        .expect("出厂的有看不了的附件带路径的两句");
    assert_eq!(
        attached.image_omitted_path,
        driver!("image-omitted-path.txt")
    );
    assert_eq!(attached.file_omitted_path, driver!("file-omitted-path.txt"));
}

/// 资源目录里缺了一份核心的字：说是哪一份，英文，写进运行日志（施工 4-9 再补四中）。
#[test]
fn a_missing_core_text_names_the_file() {
    let empty = std::env::temp_dir().join(format!("miyu-resources-empty-{}", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    let error = ResourceRoot::at(empty.clone())
        .sources_with(PersonaTexts::default())
        .unwrap_err();
    match &error {
        SourceError::Read { path, .. } => assert!(path.starts_with(&empty), "{path:?}"),
    }
    assert!(error.to_string().starts_with("cannot read "), "{error}");
    std::fs::remove_dir_all(&empty).unwrap();
}

/// 子会话的场所说明（施工 7-5）：读的是 `core/jobs/subagent-venue.txt` 的原文；没有这份的说是哪一份。
#[test]
fn the_subagent_venue_note_is_its_own_file() {
    let venue = repo().subagent_venue().unwrap();
    assert_eq!(
        venue,
        include_str!("../../../../resources/core/jobs/subagent-venue.txt")
    );
    let scratch = Scratch::new();
    let error = ResourceRoot::at(scratch.path())
        .subagent_venue()
        .unwrap_err();
    match error {
        SourceError::Read { path, .. } => {
            assert!(
                path.ends_with(Path::new("core/jobs/subagent-venue.txt")),
                "{path:?}"
            );
        }
    }
}

/// 群会话的两份（施工 O-13 中）：格式说明、空的一条写什么，读的是 `core/venues/` 下的原文；时区照交进来的；没有的说是哪一份。
#[test]
fn the_group_chat_texts_are_their_own_files() {
    assert_eq!(
        repo().group_note().unwrap(),
        include_str!("../../../../resources/core/venues/group.txt")
    );
    let chat = repo().group_chat(-300).unwrap();
    assert_eq!(chat.offset, -300);
    assert_eq!(
        chat.no_text,
        include_str!("../../../../resources/core/venues/no-text.txt")
    );
    // 群聊近况（施工 O-13 下）。
    let recent = chat.recent.expect("有群聊近况的字");
    assert_eq!(
        recent.open,
        include_str!("../../../../resources/core/venues/recent-open.txt")
    );
    assert_eq!(
        recent.omitted,
        include_str!("../../../../resources/core/venues/recent-omitted.txt")
    );
    assert_eq!(recent.budget, miyu_policy::RECENT_BUDGET);
    // 语音那一句（施工 O-33）。
    assert_eq!(
        chat.voice.as_deref(),
        Some(include_str!("../../../../resources/core/venues/voice.txt"))
    );
    let scratch = Scratch::new();
    let empty = ResourceRoot::at(scratch.path());
    for (error, file) in [
        (empty.group_note().unwrap_err(), "core/venues/group.txt"),
        (empty.group_chat(0).unwrap_err(), "core/venues/no-text.txt"),
        (
            {
                let venues = scratch.path().join("core/venues");
                std::fs::create_dir_all(&venues).unwrap();
                std::fs::write(venues.join("no-text.txt"), "x\n").unwrap();
                empty.group_chat(0).unwrap_err()
            },
            "core/venues/recent-open.txt",
        ),
        (
            {
                std::fs::write(scratch.path().join("core/venues/recent-open.txt"), "x\n").unwrap();
                empty.group_chat(0).unwrap_err()
            },
            "core/venues/recent-omitted.txt",
        ),
        (
            {
                std::fs::write(scratch.path().join("core/venues/recent-omitted.txt"), "x\n")
                    .unwrap();
                empty.group_chat(0).unwrap_err()
            },
            "core/venues/voice.txt",
        ),
    ] {
        match error {
            SourceError::Read { path, .. } => {
                assert!(path.ends_with(Path::new(file)), "{path:?}");
            }
        }
    }
}

/// 核心的几行（施工 2-7 补）：读的是 `core/permission-rule.txt`、`core/local-paths-rule.txt`、`core/machine.txt`（施工 2-7
/// 再补）的原文；没有的说是哪一份。
#[test]
fn the_core_lines_are_their_own_files() {
    let lines = repo().core_lines().unwrap();
    assert_eq!(
        lines.permission,
        include_str!("../../../../resources/core/permission-rule.txt")
    );
    assert_eq!(
        lines.local_paths,
        include_str!("../../../../resources/core/local-paths-rule.txt")
    );
    assert_eq!(
        lines.machine,
        include_str!("../../../../resources/core/machine.txt")
    );
    let scratch = Scratch::new();
    match ResourceRoot::at(scratch.path()).core_lines().unwrap_err() {
        SourceError::Read { path, .. } => {
            assert!(
                path.ends_with(Path::new("core/permission-rule.txt")),
                "{path:?}"
            );
        }
    }
}

/// 到点交进来的话的标签（施工 J-1 上）：四份都在才读得出，原样；缺一份的当没有。
#[test]
fn schedule_texts_need_all_four_files() {
    let scratch = Scratch::new();
    let root = ResourceRoot::at(scratch.path());
    assert_eq!(root.schedule_texts(), None, "一份都没有");
    for (package, name, text) in [
        ("schedule", "message-open.txt", "<task {id}>\n"),
        ("schedule", "message-close.txt", "</task>\n"),
        ("loop", "message-open.txt", "<wakeup {id}>\n"),
    ] {
        let dir = scratch.path().join("software").join(package).join("common");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), text).unwrap();
    }
    assert_eq!(root.schedule_texts(), None, "缺一份");
    fs::write(
        scratch
            .path()
            .join("software/loop/common/message-close.txt"),
        "again?\n</wakeup>\n",
    )
    .unwrap();
    assert_eq!(
        root.schedule_texts(),
        Some(miyu_policy::ScheduleTexts {
            schedule_open: "<task {id}>\n".to_string(),
            schedule_close: "</task>\n".to_string(),
            loop_open: "<wakeup {id}>\n".to_string(),
            loop_close: "again?\n</wakeup>\n".to_string(),
        })
    );
}
