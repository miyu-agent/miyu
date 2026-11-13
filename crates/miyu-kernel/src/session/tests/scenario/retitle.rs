//! 场景：人要求重起标题（施工 TT-1，`docs/blueprint/kernel/session.md`「起标题」第 9 条）。走自动起标题的那一条路，请求
//! 换成喂最近几轮的那一份（`Assembler::retitle`）；收下就回，不等模型；人起过名的、说了不自动起的、一次性的会话照样起；有一次在路上的不再发；回合正在进行、还组装
//! 不出来的等这一轮完；空闲着没东西可起的拒绝；在路上时人又改了名的不盖掉；没起成的不留到下一轮。起好的标题记成要它的那个人
//! 起的（施工 TT-2）：并进自动起的那一次的也是，以后自动起标题不再盖掉它，重启以后照样。

use super::*;
use crate::event::{ErrorClass, MetaChanged, Purpose};
use crate::session::{Reason, Titles};

/// 起标题的策略：试两次，标题最多 50 个字。
fn titled() -> Policy {
    Policy {
        titles: Some(Titles {
            tries: 2,
            chars: 50,
        }),
        ..policy()
    }
}

/// 照 [`titled`] 造的会话的替身。
fn titled_stage() -> Stage {
    Stage::new(titled, environment("~/src/miyu"), at(0))
}

/// 日志里改过的标题，照先后：是不是内核记的、改成了什么。
fn titles(stage: &Stage) -> Vec<(bool, String)> {
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::MetaChanged(MetaChanged {
                title: Some(title), ..
            }) => Some((event.by == By::Kernel, title.clone())),
            _ => None,
        })
        .collect()
}

/// 起标题的 `model.called` 有几条。
fn title_calls(stage: &Stage) -> usize {
    stage
        .model_calls()
        .into_iter()
        .filter(|called| called.purpose == Some(Purpose::Title))
        .count()
}

/// 说一句，她答一句就完。
fn one_turn(stage: &mut Stage, words: &str) {
    stage.model([Line::says("好。")]);
    stage.say(words);
}

/// 这个命令收下了，没等什么事件。
fn taken(stage: &Stage, id: &CommandId) {
    assert_eq!(
        stage.outcome(id),
        Some(&Outcome::Accepted { events: Vec::new() })
    );
}

#[test]
fn asking_feeds_the_latest_turns_and_replaces_a_title_a_person_gave() {
    let mut stage = titled_stage();
    stage.title_model([Line::says("自动的")]);
    for words in ["一", "二", "三", "四", "五"] {
        one_turn(&mut stage, words);
    }
    stage.set_meta(Some("人起的"), None);
    stage.title_model([Line::says("  重起的\n第二行不要").held()]);
    let asked = stage.retitle();
    taken(&stage, &asked);
    assert_eq!(title_calls(&stage), 1, "收下就回，模型还没说完");
    stage.release_title();
    assert_eq!(
        titles(&stage),
        [
            (true, "自动的".to_string()),
            (false, "人起的".to_string()),
            (false, "重起的".to_string()),
        ]
    );
    let [(first, auto), (latest, again)] = stage.titles() else {
        panic!("两次起标题的请求");
    };
    // 替身的组装：自动起的是第一轮的清单（`title`），人要的是最近三个回答的清单（`retitle`），没有第一轮。
    assert_eq!(*first, seq(6));
    assert_eq!(
        listed_request(auto),
        "2 message.user\n6 message.assistant\ntitle\n"
    );
    let answers: Vec<Seq> = stage
        .log()
        .iter()
        .filter(|event| matches!(event.body, Body::MessageAssistant(_)))
        .map(|event| event.seq)
        .collect();
    let [.., third, fourth, fifth] = answers[..] else {
        panic!("五个回答");
    };
    assert_eq!(*latest, fifth, "名字是最新那个回答");
    assert_eq!(
        listed_request(again),
        format!(
            "{third} message.assistant\n{fourth} message.assistant\n{fifth} message.assistant\nretitle\n"
        )
    );
    // 记账的那条是内核的；标题那条是要它的人起的（施工 TT-2），两条都不带回合编号、没有 `cause`。
    let [called, renamed] = &stage.log()[stage.log().len() - 2..] else {
        panic!("两条");
    };
    assert_eq!(called.by, By::Kernel, "{called:?}");
    assert!(matches!(renamed.by, By::Person(_)), "{renamed:?}");
    assert!(
        [called, renamed]
            .iter()
            .all(|event| event.turn.is_none() && event.cause.is_none()),
        "{called:?} {renamed:?}"
    );
    // 起过了就放下：下一轮答完不再起。
    one_turn(&mut stage, "sixth");
    assert_eq!(stage.titles().len(), 2);
}

#[test]
fn sessions_that_never_title_themselves_are_titled_when_asked() {
    for mut stage in [
        Stage::oneshot(titled, environment("~/src/miyu"), at(0)),
        titled_stage(),
    ] {
        stage.set_meta(Some("人起的"), None);
        one_turn(&mut stage, "hi");
        assert!(stage.titles().is_empty(), "自己不起");
        stage.title_model([Line::says("重起的")]);
        let asked = stage.retitle();
        taken(&stage, &asked);
        assert_eq!(titles(&stage).last(), Some(&(false, "重起的".to_string())));
    }
}

#[test]
fn with_nothing_to_title_it_is_refused_and_nothing_is_sent() {
    let mut stage = titled_stage();
    let asked = stage.retitle();
    assert_eq!(
        stage.outcome(&asked),
        Some(&Outcome::rejected(Reason::NothingToTitle))
    );
    // 策略里没有起标题的数的也是。
    let mut bare = Stage::new(policy, environment("~/src/miyu"), at(0));
    one_turn(&mut bare, "hi");
    let asked = bare.retitle();
    assert_eq!(
        bare.outcome(&asked),
        Some(&Outcome::rejected(Reason::NothingToTitle))
    );
    assert!(stage.titles().is_empty() && bare.titles().is_empty());
    // 拒了的不留着：后来答完的一轮照自动起的规矩走，只起一次。
    stage.title_model([Line::says("自动的")]);
    one_turn(&mut stage, "hi");
    one_turn(&mut stage, "again");
    assert_eq!(stage.titles().len(), 1);
}

#[test]
fn a_second_ask_joins_the_one_on_its_way() {
    let mut stage = titled_stage();
    // 自动起的那一次没排剧本：一直在路上。
    one_turn(&mut stage, "hi");
    assert_eq!(stage.titles().len(), 1);
    stage.set_meta(Some("人起的"), None);
    let first = stage.retitle();
    let second = stage.retitle();
    taken(&stage, &first);
    taken(&stage, &second);
    assert_eq!(stage.titles().len(), 1, "不再发");
}

#[test]
fn asked_during_the_first_turn_it_waits_for_the_turn_to_end() {
    let mut stage = Stage::oneshot(titled, environment("~/src/miyu"), at(0));
    stage.model([Line::says("好。").held()]);
    stage.say("hi");
    stage.title_model([Line::says("等完了起的")]);
    let asked = stage.retitle();
    taken(&stage, &asked);
    assert!(stage.titles().is_empty(), "这一轮还没答完，组装不出来");
    stage.release_model();
    assert_eq!(titles(&stage), [(false, "等完了起的".to_string())]);
    assert_eq!(stage.requests().len(), 1, "不打断这一轮");
}

/// 还没自动起过就要了（施工 TT-2）：起好的算人起的，以后答完的轮不再自动起，重启以后也不起。
#[test]
fn a_title_asked_for_counts_as_the_persons_and_is_kept_after_a_restart() {
    let mut stage = titled_stage();
    stage.model([Line::says("好。").held()]);
    stage.say("hi");
    stage.title_model([Line::says("要了起的"), Line::says("自动的")]);
    stage.retitle();
    stage.release_model();
    assert_eq!(titles(&stage), [(false, "要了起的".to_string())]);
    one_turn(&mut stage, "again");
    stage.restart();
    one_turn(&mut stage, "after");
    assert_eq!(stage.titles().len(), 1, "自动起的不再起");
    assert_eq!(titles(&stage), [(false, "要了起的".to_string())]);
}

/// 并进在路上的自动起的那一次（施工 TT-2）：起好的也算要它的人起的。
#[test]
fn joining_the_automatic_one_makes_its_title_the_persons() {
    let mut stage = titled_stage();
    stage.title_model([Line::says("自动的").held()]);
    one_turn(&mut stage, "hi");
    stage.retitle();
    stage.release_title();
    assert_eq!(titles(&stage), [(false, "自动的".to_string())]);
    assert_eq!(stage.titles().len(), 1, "不再发");
}

#[test]
fn a_person_renaming_after_asking_wins_and_a_failure_is_not_retried() {
    let mut stage = titled_stage();
    stage.set_meta(Some("人起的"), None);
    one_turn(&mut stage, "hi");
    stage.title_model([Line::says("重起的").held()]);
    stage.retitle();
    stage.set_meta(Some("又改了"), None);
    stage.release_title();
    assert_eq!(
        titles(&stage).last(),
        Some(&(false, "又改了".to_string())),
        "要了以后人又改的不盖掉"
    );
    stage.title_model([Line::fails(ErrorClass::Retryable, "503")]);
    stage.retitle();
    assert_eq!(title_calls(&stage), 2, "没起成的照记一条");
    one_turn(&mut stage, "again");
    assert_eq!(stage.titles().len(), 2, "不留到下一轮再试");
    assert_eq!(titles(&stage).last(), Some(&(false, "又改了".to_string())));
}
