//! 脚本里的 `miyu call`（施工 M-1）：三个种子里有一个，另用一串随机数，夹在原来的输入之间、不占名额，原来那串输入不跟着
//! 错开。父级多半是在跑的调用、还在跑的后台命令，偶尔是一个不在跑的（答 `gone`）；编号偶尔重来一个（不理）；工具偶尔
//! 不准、不认识，参数偶尔修不了；在过链的多半很快有结论，在跑的多半很快跑完。看守在 `watch/nested.rs`。

use super::*;
use crate::id::{NestedId, Parent};
use crate::session::NestedInput;

/// 从一组编号里挑一个；空的没有。
fn pick(rng: &mut Rng, ids: &BTreeSet<NestedId>) -> Option<NestedId> {
    let len = u64::try_from(ids.len()).ok().filter(|len| *len > 0)?;
    ids.iter()
        .nth(usize::try_from(rng.below(len)).ok()?)
        .cloned()
}

/// 这一次喂不喂、喂什么。
pub(super) fn some_nested(rng: &mut Rng, watch: &mut Watch) -> Option<Input> {
    let nested = match rng.below(10) {
        0..=2 => {
            let id = pick(rng, &watch.nesting.guarding)?;
            let module = ModuleId::parse("permissions").unwrap();
            let verdict = match rng.below(6) {
                0 => Verdict::Deny {
                    module,
                    text: "denied".to_string(),
                    human: None,
                },
                1 => Verdict::Ask {
                    module,
                    access: Access::Read,
                    rule: None,
                    detail: None,
                },
                _ => Verdict::Allow,
            };
            NestedInput::Guarded {
                at: at(51),
                id,
                verdict,
            }
        }
        3 | 4 => NestedInput::Done {
            at: at(52),
            id: pick(rng, &watch.nesting.running)?,
            error: rng.below(4) == 0,
            duration_ms: Some(3),
            result: Some(ContentHash::of(b"result")),
            bytes: Some(6),
        },
        5..=7 => {
            let mut parents: Vec<Parent> = watch
                .running
                .iter()
                .map(|call| Parent::Call(*call))
                .collect();
            parents.extend(watch.running_commands().into_iter().map(Parent::Job));
            let parent = match parents.len() {
                0 => Parent::Call(watch.some_call(rng)),
                _ if rng.below(8) == 0 => Parent::Call(watch.some_call(rng)),
                len => parents[usize::try_from(rng.below(len as u64)).unwrap()].clone(),
            };
            let next = watch.nesting.next.entry(parent.clone()).or_insert(0);
            let n = match *next > 0 && rng.below(10) == 0 {
                true => *next,
                false => {
                    *next += 1;
                    *next
                }
            };
            let name =
                ["read", "read", "shell", "write", "nope"][usize::try_from(rng.below(5)).unwrap()];
            let args = if rng.below(10) == 0 { "[1]" } else { "{}" };
            NestedInput::Call {
                at: at(50),
                id: NestedId::new(parent, n).unwrap(),
                name: name.to_string(),
                args: args.to_string(),
            }
        }
        _ => return None,
    };
    Some(Input::Nested(nested))
}
