//! 分叉读好的底子交给紧接着的载入（施工 V-39 下，`docs/construction/V-39下-分叉底子交给载入和索引.md`）：交过去的和载入
//! 自己再读父会话的（V-39 中的读法，准绳）读到的、内核载入出来的会话和动作、报的错一模一样；交过去用上了的，回合索引照底子
//! 认好的那一份和照父会话的日志到分叉点一条条认的一样。
//!
//! 父会话照种子造（同 `make.rs`），分一到三段写、有的末尾半行；普通分叉、fork 子代理。分叉以后：有的父会话又往下说了一截；
//! 交上的（这条支线、这个分叉点）照它载入，交不上的退回自己读：交给了别的支线、同一编号重新分叉以后分叉点变了、同一编号从
//! 另一个父会话（同一个分叉点、底子里有一句话不一样）分叉、交给了不是分叉出来的会话（父会话自己）；支线自己的日志坏了、
//! 支线没了，两条路报的错一样。

use std::fs;

use miyu_kernel::event::Event;
use miyu_kernel::id::{CallId, Seq, SessionId};
use miyu_session::testkit::{Script, told};
use miyu_session::{fork, fork_handing, new_id};
use miyu_store::log::read_events;

use super::make::{Dice, Make};
use super::{Start, first_line, pending_of, setup, write};
use crate::support::*;

/// 全量造多少份。
const CASES: u64 = 2500;

/// 平时造多少份：全量的 Windows debug 要两分钟上下（施工 CI-2）。设了 `MIYU_HANDED_FULL` 的照 [`CASES`]，CI 的 randomized 那一项设。
const QUICK: u64 = 250;

/// 交过去以后怎么载入。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Way {
    /// 交给这条支线、这个分叉点。
    Fits,
    /// 交给了从同一个父会话另分出来的支线。
    OtherChild,
    /// 同一编号重新分叉，分叉点变了。
    OtherUpto,
    /// 同一编号从另一个父会话（同一个分叉点，底子里有一句话不一样）分叉。
    OtherParent,
    /// 交给了父会话自己。
    NotForked,
    /// 支线自己的日志坏了一行。
    BrokenChild,
    /// 支线的目录没了。
    NoChild,
}

#[tokio::test]
async fn a_handed_base_loads_what_reading_the_parent_again_did() {
    let home = Home::new();
    let made = home.create(&Script::new([])).await;
    let created = home.log(made.id())[0].clone();
    stop(&made).await;
    let policy = match &created.body {
        miyu_kernel::event::Body::SessionCreated(body) => body.policy.to_string(),
        _ => panic!("第一条是造会话"),
    };
    let mut reached = std::collections::BTreeMap::<String, usize>::new();
    let cases = match std::env::var_os("MIYU_HANDED_FULL") {
        Some(_) => CASES,
        None => QUICK,
    };
    for seed in 0..cases {
        let mut dice = Dice(seed.wrapping_mul(0xA24B_AED4_963E_E407) | 1);
        let parent = new_id(now());
        let mut make = Make::new(&parent, first_line(&created, Start::Main), policy.clone());
        let steps = 10 + usize::try_from(dice.below(400)).expect("测试里造得出");
        make.run(&mut dice, steps);
        let mut lines = make.lines.clone();
        let calling: Vec<usize> = (0..lines.len())
            .filter(|k| lines[*k].contains(r#""tool_call""#))
            .collect();
        let cut = dice.below(4) == 0;
        if cut && let Some(k) = dice.pick(&calling) {
            lines.truncate(k + 1);
        }
        let dir = home.root.session_dir(&parent);
        write(&dir, &lines, &mut dice);
        let pending = pending_of(&lines);
        let call: Option<CallId> = match dice.below(3) {
            0 => dice.pick(&pending).copied(),
            _ => None,
        };
        let child = new_id(now());
        let at = now();
        let Ok((upto, handed)) =
            fork_handing(setup(&home, &parent, (child.clone(), at), call, seed)).await
        else {
            *reached.entry("分叉不成".to_string()).or_default() += 1;
            continue;
        };
        let primed = primed_of(&lines, upto);
        // 父会话又往下说了一截：底子到分叉点为止不变。
        if !cut && dice.below(3) == 0 {
            let more = 1 + usize::try_from(dice.below(60)).expect("测试里造得出");
            make.run(&mut dice, more);
            lines.clone_from(&make.lines);
            drop(fs::remove_dir_all(&dir));
            write(&dir, &lines, &mut dice);
        }
        let way = match dice.below(10) {
            0 => Way::OtherChild,
            1 => Way::OtherUpto,
            2 => Way::OtherParent,
            3 => Way::NotForked,
            4 => Way::BrokenChild,
            5 => Way::NoChild,
            _ => Way::Fits,
        };
        let (loaded, way) = match way {
            Way::Fits => (child.clone(), Way::Fits),
            Way::OtherChild => {
                let other = new_id(now());
                if fork(setup(&home, &parent, (other.clone(), now()), call, seed))
                    .await
                    .is_err()
                {
                    // 父会话往下说了，派的调用已经有了结果：fork 子代理分不了。
                    *reached.entry("再分叉不成".to_string()).or_default() += 1;
                    continue;
                }
                (other, Way::OtherChild)
            }
            Way::OtherUpto => {
                drop(fs::remove_dir_all(home.root.session_dir(&child)));
                let again = fork(setup(&home, &parent, (child.clone(), now()), call, seed)).await;
                match again {
                    Ok(again) if again != upto => (child.clone(), Way::OtherUpto),
                    // 父会话没往下说、在跑的一轮没结束、派的调用已经有了结果的：分叉点没变或者分不了，照交上的比。
                    Ok(_) => (child.clone(), Way::Fits),
                    Err(_) => {
                        *reached.entry("重新分叉不成".to_string()).or_default() += 1;
                        continue;
                    }
                }
            }
            Way::OtherParent => match other_parent(&home, &lines, upto, &mut dice) {
                Some(other) => {
                    drop(fs::remove_dir_all(home.root.session_dir(&child)));
                    let again =
                        fork(setup(&home, &other, (child.clone(), now()), call, seed)).await;
                    match again {
                        Ok(again) if again == upto => (child.clone(), Way::OtherParent),
                        _ => {
                            *reached
                                .entry("另一个父会话分叉不成".to_string())
                                .or_default() += 1;
                            continue;
                        }
                    }
                }
                None => (child.clone(), Way::Fits),
            },
            Way::NotForked => (parent.clone(), Way::NotForked),
            Way::BrokenChild => {
                spoil_child(&home, &child);
                (child.clone(), Way::BrokenChild)
            }
            Way::NoChild => {
                drop(fs::remove_dir_all(home.root.session_dir(&child)));
                (child.clone(), Way::NoChild)
            }
        };
        let kept = files(&home, &loaded);
        let given = told(&home.root, &loaded, Some(handed));
        restore(&home, &loaded, &kept);
        let read = told(&home.root, &loaded, None);
        assert_eq!(
            given.loaded,
            read.loaded,
            "种子 {seed}（{way:?}，{} 行，分叉点 {upto}，{call:?}）：交过去的和自己读的载入出来一样",
            lines.len()
        );
        assert_eq!(read.primed, None, "自己读的回合索引照旧后台读底子");
        let used = given.primed.is_some();
        assert_eq!(
            used,
            way == Way::Fits,
            "种子 {seed}（{way:?}）：只有这条支线、这个分叉点的用上"
        );
        if used {
            assert_eq!(
                given.primed.as_deref(),
                Some(primed.as_str()),
                "种子 {seed}：回合索引照底子认好的和照父会话的日志到分叉点认的一样"
            );
        }
        let ok = !given.loaded.starts_with("Err(");
        let key = format!(
            "{way:?}{}{}{}",
            if ok { "" } else { "（报错）" },
            if call.is_some() {
                "（fork 子代理）"
            } else {
                ""
            },
            if given.whole {
                "（整份重来）"
            } else {
                ""
            }
        );
        *reached.entry(key).or_default() += 1;
    }
    eprintln!("{reached:?}");
    for kind in [
        "Fits",
        "Fits（fork 子代理）",
        "OtherChild",
        "OtherUpto",
        "OtherParent",
        "NotForked",
        "BrokenChild（报错）",
        "NoChild（报错）",
    ] {
        assert!(
            reached.contains_key(kind),
            "一次都没走到「{kind}」：{reached:?}"
        );
    }
    assert!(
        reached
            .keys()
            .any(|key| key.starts_with("Fits") && key.contains("整份重来")),
        "交上了、底子的压缩被撤了照整份重来的一次都没走到：{reached:?}"
    );
}

/// 回合索引照底子认的：父会话的日志到分叉点为止，照先后一条条 `Priming::base`（同接上时自己读底子的）。
fn primed_of(lines: &[String], upto: Seq) -> String {
    let mut priming = miyu_recall::Priming::default();
    for line in lines {
        let Ok(event) = Event::from_line(line) else {
            continue;
        };
        if event.seq > upto {
            break;
        }
        priming.base(&event);
    }
    format!("{priming:?}")
}

/// 另一个父会话：同样的几行，分叉点以前有一句人话的字不一样。没有人话的没有。
fn other_parent(home: &Home, lines: &[String], upto: Seq, dice: &mut Dice) -> Option<SessionId> {
    let said: Vec<usize> = (0..lines.len())
        .filter(|k| {
            lines[*k].contains(r#""kind":"message.user""#)
                && lines[*k].contains("第 ")
                && u64::try_from(*k + 1).is_ok_and(|seq| seq <= upto.get())
        })
        .collect();
    let at = *dice.pick(&said)?;
    let mut other = lines.to_vec();
    other[at] = other[at].replacen("第 ", "第Ｘ ", 1);
    let id = new_id(now());
    write(&home.root.session_dir(&id), &other, dice);
    Some(id)
}

/// 支线的日志末尾接一行坏的。
fn spoil_child(home: &Home, child: &SessionId) {
    let dir = home.root.session_dir(child);
    let events = read_events(&dir).expect("支线读得了");
    let next = events.last().map_or(1, |event| event.seq.get() + 1);
    let segment = fs::read_dir(&dir)
        .expect("读得了目录")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .max()
        .expect("有一段");
    let mut text = fs::read_to_string(&segment).expect("读得了");
    text.push_str(&format!("{{\"seq\":{next},\"kind\n"));
    fs::write(&segment, text).expect("写得进");
}

/// 会话目录里的每个文件（载入可能截掉末尾半行：两次载入从同一份起）。
fn files(home: &Home, id: &SessionId) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let Ok(entries) = fs::read_dir(home.root.session_dir(id)) else {
        return Vec::new();
    };
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .map(|path| {
            let bytes = fs::read(&path).expect("读得了");
            (path, bytes)
        })
        .collect();
    files.sort();
    files
}

/// 照 [`files`] 记下的写回去。
fn restore(home: &Home, id: &SessionId, kept: &[(std::path::PathBuf, Vec<u8>)]) {
    if kept.is_empty() {
        return;
    }
    drop(fs::remove_dir_all(home.root.session_dir(id)));
    fs::create_dir_all(home.root.session_dir(id)).expect("建得了");
    for (path, bytes) in kept {
        fs::write(path, bytes).expect("写得回");
    }
}
