## 施工单 E-M 上：她知道自己是什么模型

状态：合了（2026-10-23）。项目主人 2026-10-22 定：模型、harness、思考程度进上下文，三样都要，没写的强度写 `default`；模型名、思考强度、会话日志的目录也进命令的环境变量；10-23 定预设进模型那一块、只写编号。下半（session_usage 的用时和缓存命中率、等人的时间不算用时）在 E-M 下。图纸 `docs/blueprint/kernel/request.md`「事实」、`docs/designs/26-提示词.md`。

### 定的（技术题，主会话定）

1. **模型不进 system**：会话中途能换模型、强度、预设，放 system 换一次掰一次缓存。进尾巴的事实 `core/facts/model.txt`，和环境那一块一样每个边界查：同类最近一块逐字节相同的不注入，换了任一样追加一块，前面的缓存不动；压缩、撤销拿走了的下一个边界重新注入。
2. **四格**：供应商照配置里的显示名（没写的照编号），模型名，强度（配置里写了的那一档，没写的 `default`），预设的编号（不用预设的 `none`）。供应商名是人起的，换字段时照常转义。
3. **harness 进 system**：`core/agent-version.txt`「You run in miyu-agent {version}.」，`{version}` 换成核心编进去的版本；一个会话里不变，有没有工具都带，接在核心的几行最后。文件不叫 `harness.txt`：`core/harness/` 是别的 harness 发来的话的标签。
4. **限额带身份**：执行器解析这一轮时算好 `Identity`，随 `Limits` 交给内核；身份变了的置 `turn.refresh`，下一次请求前重新查事实。预设执行器那一层补。
5. **环境变量**：命令和脚本都拿到 `MIYU_MODEL`、`MIYU_EFFORT`（同模型那一块）；`MIYU_SESSION_LOG`（会话日志的目录）只给属主是管理员的会话（`06-多用户与身份.md` U25：普通用户看不到运行它的那台电脑）。核心环境里原有的 `MIYU_*` 照旧不传。
6. **旧快照**：没有模型那份模板的不注入，没有 harness 那一行的不拼，一个字节不变。

### 做了什么

- 内核：`facts/model.rs`（`Identity`、`with_model`、`model`），边界多查一块，`Limits.identity`、`take_limits`。
- 策略：`FactTexts.model`、`CoreLines.harness`；资源读 `model.txt`、`agent-version.txt`。
- 会话：`route/identity.rs` 算身份，`actor/model.rs` 补预设、顺手交给工具（`Callers` 记模型和日志目录）；`ToolKit.session_log` 照属主是不是管理员给。
- 工具：`Caller` 多 `model`（`InUse`）、`log`；基础系统 `shell/env.rs` 的 `told` 给命令、脚本设上三个变量。
- 测试：内核模型那一块 6 条（造时查、四格、转义、排在会话编号后面、同一个不再注入换了追加、没身份没模板没有）；场景 4 条（第一轮带、轮间换了追加、回合中途换了下一次请求前追加、交同一个不追加、没身份不注入）；环境变量一条；会话层属主不是管理员的没有日志目录、调用带着模型和日志目录。变异 4 个全抓住（去掉边界上的模型那一块、中途换了不重查、不设 `MIYU_SESSION_LOG`、非管理员也给日志目录）。
