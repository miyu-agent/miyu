## 施工 CK-1：默认人格、默认预设指着没有的

状态：做完，等验收（分支 `step/PRE-A`）。来源：主会话 2026-11-17 交来（发版前「字和检查」一批）。

### 查到的

- `persona.default`、`preset.default` 的类型是名字（`kind: name`），只查写法，不查有没有。`miyu config set preset.default 不存在的` 照收，`miyu check` 说没有问题，开会话时才报（预设回 `unknown_preset`；人格当没设，只在 `core.log` 记一条 `WARN`）。
- 软件包设置项、终端会话的 `terminal.persona`、`terminal.preset` 早就是「选人格」「选预设」两种类型（施工 PC-3、PC-4）：`check` 照核心这时有的人格、预设另查，没有的报警告「`<键>` 写的人格 `<名>` 现在没有：建了这个人格以后就能用」（`unknown_persona`、`unknown_preset`）。是**警告、不挡**：人格、预设可能以后才建。`config.set` 那时什么都不说。

### 怎么改

1. `persona.default`、`preset.default` 的类型改成选人格、选预设（`miyu-endpoint/src/settings.rs`），同终端会话那两项：`config.schema` 的 `type` 是 `persona`、`preset`，头照列表让人挑；`check` 照现有的查，没有的报同一条警告（三种语言的字是现成的）。出厂的预设 `full` 算有。
2. `config.set`（人改的，扩展改自己包的不管）写成了以后，改了的几项里选人格、选知识库、选预设指着现在没有的，回应多一格 `problems`：同 `check` 的那一条警告，照连接的语言说；一条都没有的不带这一格（`config/set/unknown.rs`）。照收、不挡，同软件包设置项（照它一致：那一句是警告）。软件包的设置项、终端会话那两项也一样受益。
3. `miyu config set` 照回应的 `problems` 在结果行后面一条接一行 `miyu: warning: …`（中文 `miyu: 警告：…`）。

不选「`config set` 当场拒」：软件包设置项那一句是警告，人格、预设本来就可以先指后建（例如先设默认人格、再装带人格的包）。

### 蓝图改哪几节

`config.md`：类型表「人格、知识库、预设」一行；配置项表 `persona.default`、`preset.default` 两行。

### 验收

1. 改前红（换回 main 的 `settings.rs`、`methods.rs` 跑）：
   - `miyu-endpoint` 的 `default_names::check_warns_when_the_defaults_name_what_is_not_there`：两项的 `type` 是 `persona`、`preset`，`check` 报 `unknown_persona`、`unknown_preset`。
   - `default_names::setting_a_default_that_is_not_there_warns_in_the_reply`：`config.set preset.default = gone` 照收，回应的 `problems` 是英文那句；设成 `full`、有的人格不带 `problems`。
   - `miyu` 的 `config_keys::a_default_that_is_not_there_is_set_with_a_warning_and_checked`：英文界面下 `miyu config set` 结果行后面接 `miyu: warning: preset.default names preset gone, …`，`miyu check` 报两条警告、退出码 0。
2. 配置样本（`internal/designs/samples/config/`）里两项的类型跟着变。
3. 门禁全过等（同本批）。

### 风险

- `config.set` 的回应多一格 `problems`：头照字段取，不认识的不管；要在设置页上提示的，照它画。
- `config.schema` 里两项的 `type` 从 `name` 变成 `persona`、`preset`：头已经认这两种（终端会话那两项），设置页从文字框变成照列表挑。
