## 施工单 S-T：会话的工具（`session.tools`）

状态：合了（2026-10-23）。终端会话转来（2026-10-22）项目主人要的 `/tool-list`：看这个会话里她能用的工具；只列她下一轮真能用的（发给模型的那一份），按来源分组；还没开会话（或者 `/new` 以后还没说第一句）的也能看，照新会话会用的人格、预设列。第一版只列名字，不做给人看的说明（项目主人同一天定）。图纸 `docs/blueprint/protocol.md`「`session.tools`」。

### 定的（技术题，主会话定）

1. **先后**：照发给模型的先后，就是名字的字节序（组装请求时照名字排）；分组由头照 `source` 分。
2. **写了 `session` 的**：问那个会话（`Handle::tools`）。它照回合开始换快照那一套（`actor/persona.rs` 的 `look`）看一遍，人格、预设、工具目录、知识库换了的照换了以后的算，没换的照现在的快照；只看不换，换不换照旧等回合开始。造了还没说话的会话照它现在的人格、预设、模型算。
3. **没写的**：照新会话会拿到的算，和 `session.create` 照同一份拼：端点的 `sessions/setup.rs`（从 `Sessions::create` 挪出来）找预设、人格，会话那一层的 `open/face.rs`（从 `create` 挪出来）拼工具面；只拼不造，什么都不写。`persona`（`null` 是无人格）、`preset` 同 `session.create`，写了 `session` 的不收这两格（`bad_params`）。
4. **要先加载的**：只露名字的写 `loaded: false`；有它们的多一件 `load_tools`（`miyu_policy::offered`，和 `loading::stub` 走同一条路）。
5. **来处**：MCP 的照服务名（`kind: mcp`）；脚本照脚本的显示名、一句说明（照连接的语言挑，`kind: script`）；别的照工具目录记的包（没记的是基础系统），包名照清单，内置的包 `kind: builtin`、别的 `package`。技能不是工具，不出现。
6. **权限**：同 `session.recap`（人都能调，扩展要 `events.read`）。

### 做了什么

- `miyu-policy`：`offered`（和 `loading::stub` 走同一条路，`Offered{name, loaded}`）。
- `miyu-session`：`Handle::tools`、actor 的 `Message::Tools`（`actor/persona/next.rs` 照 `look` 看一遍、只看不换）；造会话拼工具面那一段挪进 `open/face.rs`，`preview` 和 `create` 共用。
- `miyu-endpoint`：`session.tools`（`session_tools.rs`）；`Sessions::create` 找预设、人格、拼 `Create` 那一段挪进 `sessions/setup.rs`，预览新会话和造会话共用。
- 测试：`miyu-policy` 一条（发的样子和 `stub` 一致）；端点两条（会话的照名字排、带来处，新会话的预览和真造出来的一样；拒的几种）。变异 5 条杀 4 条；活下来的是「会话中途换了预设、还没到下一轮时照换了以后的算」那一支，测试里没有中途换预设的会话，留着没补。
