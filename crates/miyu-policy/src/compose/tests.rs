use super::*;

#[test]
fn pieces_are_trimmed_and_joined_with_a_blank_line() {
    assert_eq!(system(&["人设\n", "", "  \n", "场所\n\n"]), "人设\n\n场所");
    assert_eq!(
        system(&["You are a helpful software engineer.\n"]),
        "You are a helpful software engineer."
    );
    assert_eq!(system(&[]), "");
    // 开头的空白是人格自己写的，照留。
    assert_eq!(system(&["  缩进的第一行\n"]), "  缩进的第一行");
}

#[test]
fn the_venue_note_follows_the_persona_after_a_blank_line() {
    let snapshot = crate::test_support::engineer();
    let persona = snapshot.system.clone();
    let child = snapshot.with_venue("You are a subagent.\n");
    assert_eq!(child.system, format!("{persona}\n\nYou are a subagent."));
    let blank = crate::test_support::engineer().with_venue("\n");
    assert_eq!(blank.system, persona, "空的说明不留空行");
}

/// 核心的几行（施工 2-7 补）：三句，一行一句，排在人设、场所说明后面，空一行；本机是什么系统那一句的 `{os}` 换成系统的名字
/// （施工 2-7 再补）。
fn lines() -> CoreLines {
    CoreLines {
        permission: "Permission rule.\n".to_string(),
        local_paths: "Local paths rule.\n".to_string(),
        machine: "Runs {os}.\n".to_string(),
        harness: String::new(),
        style_lock: String::new(),
        clicall: String::new(),
    }
}

/// 一件工具：有它，工具面就不是空的。
fn a_tool() -> crate::tools::ToolEntry {
    serde_json::from_str(
        r#"{"name":"read","description":"Read.","parameters":{"type":"object"},"access":"read"}"#,
    )
    .unwrap()
}

#[test]
fn the_core_lines_come_after_the_persona_and_the_venue() {
    let persona = crate::test_support::engineer().system;
    let main = crate::test_support::engineer()
        .with_tools(vec![a_tool()])
        .with_core_lines(&lines(), "Arch Linux", Shown::default());
    assert_eq!(
        main.system,
        format!("{persona}\n\nPermission rule.\nLocal paths rule.\nRuns Arch Linux.")
    );
    let child = crate::test_support::engineer()
        .with_tools(vec![a_tool()])
        .with_venue("You are a subagent.\n")
        .with_core_lines(&lines(), "Arch Linux", Shown::default());
    assert_eq!(
        child.system,
        format!(
            "{persona}\n\nYou are a subagent.\n\nPermission rule.\nLocal paths rule.\nRuns Arch Linux."
        )
    );
}

#[test]
fn a_session_without_tools_has_no_permission_or_machine_line() {
    let persona = crate::test_support::engineer().system;
    let bare =
        crate::test_support::engineer().with_core_lines(&lines(), "Arch Linux", Shown::default());
    assert_eq!(bare.system, format!("{persona}\n\nLocal paths rule."));
}

/// 带角色扮演提示的人格：快照里是拼好的一块（包装、去掉末尾空白的原文、换行、收尾），造出的策略第一轮就注入它。
fn reminding(reminders: &str) -> Snapshot {
    let mut sources = crate::test_support::sources();
    sources.persona.reminders = reminders.to_string();
    sources.reminder = Wrap {
        open: "<persona-reminder>\n".to_string(),
        close: "</persona-reminder>\n".to_string(),
    };
    compose(Some("miyu"), sources, true)
}

#[test]
fn a_reminder_is_wrapped_into_the_snapshot_and_reaches_the_facts() {
    let snapshot = reminding("  Stay soft.\n\n");
    let block = "<persona-reminder>\n  Stay soft.\n</persona-reminder>\n";
    assert_eq!(snapshot.reminder.as_deref(), Some(block));
    let fact = snapshot
        .policy()
        .unwrap()
        .facts
        .reminder(&miyu_kernel::history::History::default())
        .unwrap();
    assert_eq!(fact.text, block);
    let read = Snapshot::from_bytes(&snapshot.to_bytes()).unwrap();
    assert_eq!(read, snapshot, "存得回来");
}

#[test]
fn a_blank_reminder_is_none_and_the_snapshot_is_as_before() {
    assert_eq!(reminding(" \n\t\n").reminder, None);
    let engineer = crate::test_support::engineer();
    assert_eq!(engineer.reminder, None);
    let bytes = String::from_utf8(engineer.to_bytes()).unwrap();
    assert!(!bytes.contains("reminder"), "没有的不写：{bytes}");
    assert!(
        engineer
            .policy()
            .unwrap()
            .facts
            .reminder(&miyu_kernel::history::History::default())
            .is_none()
    );
}

#[test]
fn the_style_lock_ends_the_system_of_a_persona_with_a_reminder_only() {
    let lock = "<style-lock>Stay.</style-lock>\n";
    let main = reminding("Stay soft.")
        .with_tools(vec![a_tool()])
        .with_core_lines(&lines(), "Arch Linux", Shown::default())
        .with_style_lock(lock);
    assert!(
        main.system
            .ends_with("Local paths rule.\nRuns Arch Linux.\n\n<style-lock>Stay.</style-lock>"),
        "{}",
        main.system
    );
    let plain =
        crate::test_support::engineer().with_core_lines(&lines(), "Arch Linux", Shown::default());
    assert_eq!(
        plain.clone().with_style_lock(lock).system,
        plain.system,
        "没有角色扮演提示的不带"
    );
}

/// 人格的指纹（施工 P-1 再补）：同样的字同样的指纹，三份里改了哪一份都变；拼进快照。
#[test]
fn the_digest_follows_the_three_persona_texts() {
    let base = crate::test_support::sources().persona;
    let digest = base.digest();
    assert_eq!(base.clone().digest(), digest);
    let mut persona = base.clone();
    persona.persona.push('x');
    let mut examples = base.clone();
    examples.examples = crate::persona::read_examples("user: a\nassistant: b\n").unwrap();
    let mut reminders = base.clone();
    reminders.reminders = "Stay.".to_string();
    for changed in [persona, examples, reminders] {
        assert_ne!(changed.digest(), digest);
    }
    assert_eq!(crate::test_support::engineer().persona_digest, Some(digest));
}

/// 换快照只许人格的那几格不一样（施工 P-1 再补）：核心的字、工具面、人格编号变了的都不算。
#[test]
fn only_the_persona_and_preset_parts_may_differ_for_a_swap() {
    let old = crate::test_support::engineer()
        .with_tools(vec![a_tool()])
        .with_core_lines(&lines(), "Arch Linux", Shown::default());
    let mut sources = crate::test_support::sources();
    sources.persona.persona = "You are Miyu.\n".to_string();
    sources.persona.reminders = "Stay.".to_string();
    let new = compose(Some("engineer"), sources, true)
        .with_tools(vec![a_tool()])
        .with_core_lines(&lines(), "Arch Linux", Shown::default())
        .with_style_lock("<lock/>");
    assert!(old.swappable(&new));
    let mut core = new.clone();
    core.core.facts.env = "<e/>\n".to_string();
    assert!(!old.swappable(&core), "核心的字变了");
    let mut tools = new.clone();
    tools.tools.clear();
    tools.preset = Some(crate::preset::PresetPin {
        id: "dev".to_string(),
        off: vec!["basesystem".to_string()],
        digest: None,
    });
    assert!(
        old.swappable(&tools),
        "工具面、预设可以变（施工 P-2 下：换预设时重新筛过）"
    );
    let mut memory = new.clone();
    memory.memory = Some("off".to_string());
    assert!(!old.swappable(&memory), "记忆的范围钉在会话上");
    let mut attended = new.clone();
    attended.attended = false;
    assert!(!old.swappable(&attended));
    let mut other = new;
    other.persona = Some("miyu".to_string());
    assert!(!old.swappable(&other), "不是同一个人格");
}

#[test]
fn without_the_core_lines_the_system_is_as_before() {
    let tooled = crate::test_support::engineer().with_tools(vec![a_tool()]);
    assert_eq!(tooled.system, "You are a helpful software engineer.");
    let empty = CoreLines {
        permission: "\n".to_string(),
        local_paths: String::new(),
        machine: String::new(),
        harness: String::new(),
        style_lock: String::new(),
        clicall: String::new(),
    };
    assert_eq!(
        tooled
            .clone()
            .with_core_lines(&empty, "Arch Linux", Shown::default())
            .system,
        tooled.system,
        "空的几行不留空行"
    );
}

/// 无人格（施工 P-4 上）：快照不写 `persona`、没有人格的指纹，system 里只有核心的那段；带人格的字节照旧写着它。
#[test]
fn without_a_persona_the_snapshot_names_none_and_says_no_character() {
    let mut sources = crate::test_support::sources();
    sources.persona = PersonaTexts::default();
    let nobody = compose(None, sources, true);
    assert_eq!((&nobody.persona, &nobody.persona_digest), (&None, &None));
    let bytes = String::from_utf8(nobody.to_bytes()).expect("是字");
    assert!(!bytes.contains("\"persona\""), "{bytes}");
    assert!(
        !nobody.system.contains("software engineer"),
        "{}",
        nobody.system
    );
    let created = nobody.session_created(
        miyu_kernel::id::AccountId::parse("alice").unwrap(),
        miyu_kernel::id::VenueId::parse("local").unwrap(),
        miyu_kernel::event::Permission {
            level: miyu_kernel::event::Level::Workspace,
            read_only: false,
        },
    );
    assert_eq!(created.persona, None);
    let named = String::from_utf8(crate::test_support::engineer().to_bytes()).expect("是字");
    assert!(named.starts_with("{\"persona\":\"engineer\","), "{named}");
}

#[test]
fn the_clicall_line_is_its_own_paragraph_only_when_told() {
    let tooled = crate::test_support::engineer()
        .with_tools(vec![a_tool()])
        .with_core_lines(&lines(), "Arch Linux", Shown::default());
    let mut with = lines();
    with.clicall = "You can write programs that call tools.\n".to_string();
    assert_eq!(
        tooled.clone().with_clicall(&with, false).system,
        tooled.system,
        "没有能调的不带"
    );
    assert_eq!(
        tooled.clone().with_clicall(&with, true).system,
        format!(
            "{}\n\nYou can write programs that call tools.",
            tooled.system
        )
    );
}

use miyu_kernel::facts::Gates;

/// 预设关了的那几行不带（施工 C-11）：权限那句跟着权限级别，本机文件的路径跟着工作目录，系统、harness 各自一个开关。
#[test]
fn a_preset_can_leave_out_each_core_line() {
    let persona = crate::test_support::engineer().system;
    let with_harness = CoreLines {
        harness: "Runs in miyu.\n".to_string(),
        ..lines()
    };
    let compose = |shown: Shown| {
        crate::test_support::engineer()
            .with_tools(vec![a_tool()])
            .with_core_lines(&with_harness, "Arch Linux", shown)
            .system
    };
    let all = "Permission rule.\nLocal paths rule.\nRuns Arch Linux.\nRuns in miyu.";
    assert_eq!(compose(Shown::default()), format!("{persona}\n\n{all}"));
    let off = Shown {
        facts: Gates {
            permission: false,
            workdir: false,
            ..Gates::default()
        },
        ..Shown::default()
    };
    assert_eq!(
        compose(off),
        format!("{persona}\n\nRuns Arch Linux.\nRuns in miyu.")
    );
    let off = Shown {
        machine: false,
        harness: false,
        ..Shown::default()
    };
    assert_eq!(
        compose(off),
        format!("{persona}\n\nPermission rule.\nLocal paths rule.")
    );
    // 时间、会话编号、模型不管 system。
    let off = Shown {
        facts: Gates {
            time: false,
            session: false,
            model: false,
            ..Gates::default()
        },
        ..Shown::default()
    };
    assert_eq!(compose(off), format!("{persona}\n\n{all}"));
}
