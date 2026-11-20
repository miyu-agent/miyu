## 施工单 SEC-5：关起来的会话碰不到数据根，补两条防御测试

状态：做完了，等合并（分支 `step/SEC-5`，从 main 开）。文档会话核对代码时提出，主会话 2026-11-20 转来。

### 要确认的事

扩展的工具在规格里声明 `paths` 以后，核心按声明从参数里取出要碰的路径，和内置的文件工具走同一张判法表。施工 RO-3 以后，本人的会话在完全放开时可以读写数据根，这是项目主人定的。需要确认的是：群聊这类关起来的会话（`confined`）里，带 `paths` 的扩展工具指向数据根时，读和写是不是仍然都被拒绝。如果漏了，群里的人就可以让 AI 读出密钥文件。

### 核实的结果

成立，没有漏洞，没有修改程序。

- 读代码：权限策略的入口 `Guard::paths`（`crates/miyu-session/src/guard.rs`）对每一条要碰的路径做同样的判断，路径来自 `Tool::targets`，不看是哪件工具。内置的文件工具自己报路径；扩展工具的 `targets` 由 `crates/miyu-endpoint/src/provide/declared.rs` 按声明的 `paths` 从参数里取。关起来的会话里，真实位置不在这一轮的工作区、也不在加进来的目录里的一律拒绝（`guard/confined.rs` 的 `inside`），读和写都是；只有读开着的技能文件夹例外。这个判断在按权限级别判之前，所以完全放开也不放行。
- 测试：原来的 `guard_confined.rs` 只覆盖了「工作区以外的普通目录」，没有专门覆盖数据根，也没有覆盖「工作区本身在数据根里」的情况（场所会话的默认工作区在 `root/workspace/` 下）。这一步补了两条，在现在的代码上通过。
- 这两条测试确实守得住：临时把判断改成「落在数据根那一片的不拒」（模拟漏掉数据根的写法），两条都变红；改回去以后通过。

### 改成什么样

只加测试，`crates/miyu-session/tests/guard_confined.rs`：

- `a_confined_session_touches_nothing_in_the_data_root`：工作区在数据根外面的群会话，读、写数据根里的密钥文件 `etc/private/secrets.toml` 和自带技能 `root/skills/notes/SKILL.md`，四次调用都被拒绝，工具一次都没有运行。
- `a_confined_session_working_inside_the_data_root_stays_in_its_own_place`：工作区是 `root/workspace/qq-group-1` 的群会话，读写自己工作区里的文件放行；读写另一个群的工作区、密钥文件都被拒绝。

测试用的是报参数里路径的假工具。扩展工具的声明怎么变成路径由 `crates/miyu-endpoint/tests/provide_paths.rs` 守着（真的扩展、真的核心），两处合起来覆盖整条路径。

蓝图 `session/guard.md` 的测试表那一行跟着补上。

### 不做什么

- 不加「真的扩展工具在真的群会话里」的端到端测试：要搭一个通讯平台的桥和声明了场所的扩展，而判断的那一段代码和上面两条测试走的是同一条。

### 验收

- 两条新测试通过；临时改坏判断时两条都变红。
- 门禁、Windows 版 clippy、三个平台的 CI 全部通过。

### 怎么实测验收

这一步没有修改程序，没有可以实测的新行为。验收可以运行：

```bash
cargo test -p miyu-session --test all guard_confined
```
