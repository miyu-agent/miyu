# AE-7：升级到一半的包起来时放回去

状态：已完成（2026-10-30）。发布前第二轮审查查出（R2-4，材料在 `~/.cache/miyu-audit/2026-10-30/round2-core.md`），core1 排给 99 做，项目主人同意。

## 目的

升级装上的包时，原来那一份先挪到 `.<编号>.old`、再把暂存的 `.<编号>.new` 换进来。两次改名之间核心被杀，或者第二次改名失败，`<编号>/` 就没了，重启也没人放回去，包从列表里消失。做成以后：换进去失败当场放回去；核心起来时把上一次剩下的收拾好，包照旧在。

## 蓝图改哪几节

- `packages.md`「装卸」第 1 条：写明暂存处、备份处的名字；换进去失败当场放回；新加「起来时收拾」一款：`<编号>/` 不在、`.<编号>.old` 在的放回去，别的 `.old`、`.new` 删掉，认不出的名字不碰，记哪几行日志。测试表一行。
- `core.md`「起来的先后」第 5 步：建好默认工作区以后、读清单之前收拾一遍（`leftovers.rs`）；「在哪」一行；测试表一行。

## 不做什么

- 不改装包的两次改名本身（换成原子交换要分平台，Linux 的 `renameat2` 别处没有）：剩下的由起来时收拾兜住。
- 不碰 `packages/` 里认不出的名字，人放的东西照留。

## 验收

1. 真核心 `crates/miyu/tests/leftovers.rs` 的 `an_upgrade_cut_off_between_the_renames_is_put_back_when_the_core_starts`：修前红（「原来那一份放回去了」，`pudding/` 不在），修后绿。
2. `miyu-store` 的 `an_upgrade_cut_off_between_the_renames_is_put_back`（第二轮审查补丁里那条改成走 `install::recover`）、`recovering_leaves_what_it_does_not_recognize`。
3. 手写变异 3 个：不放回去、不查编号写法、本体在也放回，都抓住。
4. 本地门禁、Windows clippy、三平台 CI。

## 风险

- 起来时以为是剩下的、其实正在装：一个数据根只跑一个核心（单实例锁），收拾在读清单、开连接之前，这时没有在装的。
- 放不回去（权限、盘满）：记 `WARN package leftover kept`，不耽误起来，下一次起来再试。
