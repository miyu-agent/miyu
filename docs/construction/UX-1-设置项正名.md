## 施工 UX-1：设置项正名，软件包只留一个「启用」

状态：做完，等合（分支 `step/UX-1`）。项目主人 2026-11-17 定。

### 为什么

主人验收时撞上：设置页「把这个包关掉」（`packages.<编号>.off`，施工 U23 末上，多用户时「对某个人隐藏这个包」）和软件包真正的开关（`extensions.json` 的 `on`、协议 `package.enable`、`package.disable`、进程起停）是两个开关管一件事。主人在设置里拨了 `off = false` 以为开了，包其实没起。原话：「我 off 掉一个 off 代表开？这完全不符合直觉，难道不应该是是否开启，然后给一个 bool 吗？」也批了设置项名字口语化（「为什么又要用『把这个包关掉』这么口语化的配置项取名」）。多用户 10-28 已经拆了，「只对你自己」没有意义。

主人定：

1. 只留一个「启用」：删掉 `packages.<编号>.off` 这一层（配置键、清单、会话里照它扣包的逻辑、字表）；设置页的包开关就是真正的启停，名字「启用」，`true` 是开。发版前不留兼容：写了 `off` 的照不认识的键警告（同 PV-RM 删 `sandbox.private.*`）。
2. 设置项名字照主人给的表改成软件术语，英文、日文照同一标准写成软件里的说法。
3. 清死名字、说明里的多用户残留、反向布尔；命名规矩记进蓝图并加检查。

### 改了什么

**上：开关收拢**

1. 删：`miyu-session` 的 `packages_off`（`ITEMS`、`item_for`、`of` 和测试）；`miyu-config` 的 `package::settings::OFF`；`miyu-policy` 的 `PresetFile::switched_off`、`switching_off`；`miyu-endpoint` 的 `packages/off.rs`（`Core::packages_off`）、`packages/own.rs` 里「去掉关了的」那一份（`Core::packages_of`、`Visible` 缓存，调用处换成 `system_packages`）、`Core::scripts_for`（换成 `scripts`）、命令目录、命令模板、MCP 提示词、预设、`config.schema`、`config.get` 里照它筛的几处；`miyu-core`、`miyu-endpoint` 登记清单里那一行，`settle_with` 给每个包拼的那一项。
2. 加：`miyu-config` 的 `package::enabled`：样子 `packages.<id>.enabled`、带占位的一项（只给字表对账，不进清单）、每个包拼一次留到退出的那一项（`item_for`）、从键认包（`id_of`）。`miyu-endpoint` 的 `config/enabled.rs`：
   - `config.schema`：这一刻有开关的包（同 `package.list` 带 `enabled` 的，卸掉了的出厂包也算）各一项，`bool`、`toggle`、`now`，「软件包」页那个包那一组，排在包自己的配置项前面；`keys` 写它的照认。
   - `config.get`：`{"origin": {"layer": "package"}, "value": <开没开>}`。
   - `config.set`（人改的那一路）：改它的几项照 `package.enable`（不带 `approve`）、`package.disable` 开关包（`packages/switch.rs` 的 `turn`），不写配置文件；同一条里别的项先照常写；只收开关值，`unset`、重复的键 `bad_params`，`expect` 不看。扩展改它照旧 `local_only`（`declared_by` 不算它）；后台页的桥（`backstage/host/bridge.rs`）不把它算成包自己的项。
   - 命令行（`miyu-cli`）：`miyu config set packages.<编号>.enabled true` 照开关包，来源 `package` 不报「被压着」；`miyu config get` 不写键的不印「启用」那几项（不在配置文件里），点名问的照印。
   - 它不在配置清单里：配置文件里写 `packages.<编号>.enabled` 一样是不认识的键，生成的 Schema、参考文件没有它（样本里每个出厂包的 `off` 那一段跟着没了）。
3. 字表三语：`packages.<id>.off` 换成 `packages.<id>.enabled`「启用 / Enabled / 有効」，说明只说事实。

**下：改名、清理、规矩**

1. 改名（`resources/core/human/{zh,en,ja}.json`）照主人的表，外加三处不合规矩的：`models.catalog.every`「多久更新一次」→「更新间隔」、`profile.about` 英文日文（More about you / あなたについての補足 → Additional details / 補足）、组 `profile`「关于你」→「个人资料 / Profile / プロフィール」。`sandbox.tools.<name>` 施工 LV-2 已经删了，字表里也没有，不用改。
2. 死名字：查过三份字表每一项都在核心清单里（`every_language_names_every_item_and_nothing_more` 本来就拦多出来的），`sandbox.private.*` PV-RM 时已经删干净，这一步没有别的死名字可删。
3. 多用户残留：说明里只有 `packages.<id>.off` 那一句（「只对你自己……别人照旧」，随项删了）。`net.internal_domains` 的「别人（群里的人、陌生人）」、`commands.<id>.access` 的「你和群管理员」说的是群聊里的人，不是多用户，留着。
4. 反向布尔：核心登记的开关里只有 `packages.<id>.off` 是 `true` 表示关，已换成 `…enabled`。别的开关（`permission.confirm.*` 开着才问、`permission.start_read_only` 开着才只读……）都是正着说。枚举里的 `off`（`log.level`、`commands.<id>.access`、`models.embedding`）不是开关，不动。
5. 规矩：`miyu-config` 的 `naming`，`miyu-core` 的 `tests/settings_names.rs` 拿它查核心登记的项、包的「启用」、出厂的包的项在三份字表里的名字：开关的键不以 `off`、`disable(d)` 结尾、不以 `no_`、`not_`、`disable_`、`hide_`、`skip_` 开头；名字不带人称、疑问、把字句、句末标点、「AI」；开关的名字不带「不」「禁用」「隐藏」这类反着说的字。名词短语靠人审。蓝图 `config.md`「给人看的字」加「命名的规矩」一段。
6. 样本：`MIYU_SAMPLE_WRITE=1` 重写 `docs/designs/samples/config/` 四份，`config.md` 嵌的参考文件同步。
7. 文档：蓝图 `config.md`（第一条去掉的、协议 `config.schema`、`config.get`、`config.set`、名字表、命名的规矩）、`packages.md`「关掉某个包」、`presets.md`；协议参考中英 `config`。`docs/guide` 不碰（照 PV-RM 的先例另起 DOC 步）。

### 验收

1. 改前红：`miyu-core` `settings_names::every_name_is_a_term_and_every_switch_says_on`（把三份字表换回 main 的跑过：报「谁能用这条命令」「核心空闲多久退出」「我的平台账号」……）；`settings_removed::the_package_off_key_only_warns_as_unknown`（main 上 `off` 在清单里、照收不报）；`miyu-endpoint` `package_enabled` 两条（main 上没有「启用」）。
2. 跟着改的测试：`config.rs` 两条（出厂的包各一项「启用」，排在最后的「软件包」页）、`extension_config.rs`（扩展改自己的「启用」回 `local_only`）、`page_host.rs`（后台页不算它）、`miyu-core` `tools.rs`、`settings.rs`；删了 `package_off.rs`、`package_off_more.rs`、`packages_off/tests.rs`、`miyu-policy` 的两条。
3. 门禁十一项、`random::` 长跑、Windows 版 clippy、`cargo xtask prompts`、`cargo xtask logs`。
4. 实测（沙箱数据根，调试版核心）：装一个手动开的扩展 `tick-pkg`（程序只等着）；`config.schema` 那一项 `name` 是「启用」、`type` 是 `bool`，排在 `tick-pkg.interval` 前面，一项 `.off` 都没有；`config.get` 是 `{"origin":{"layer":"package"},"value":false}`；`config.set` true 后日志 `extension started package=tick-pkg`、进程在，false 后 `extension exited`、进程没了，`etc/config.toml` 没生成。`miyu config set packages.tick-pkg.enabled true` 印「已设置 … = true（立即生效）」。手写 `[packages.tick-pkg] off = false`，`miyu check` 报「没有 packages.tick-pkg.off 这一项。已忽略此行。」。
5. 网页头（miyu-web main `f346c39a`）：包的信息页「设置」底下画出「启用」和说明，点开包起来、头部开关跟着变开；改名的几项照新名字画。两处要头改：「启用」旁边挂着「默认」小标签（头把来源 `package` 当默认）；在头部开关关掉包以后，「启用」不跟着变（核心不为包开关推 `config.changed`，头要照 `package.changed` 重读，或者在包的信息页不画这一项）。

### 风险

- 扩展关了以后，它带的技能、脚本、命令模板照旧在（原来 `off` 会一起扣下）：真正的开关只停进程和它带的服务。要不要让关着的扩展连带的技能脚本一起不列，另报拍板。
- 网页头的两处（「默认」小标签、头部开关关了「启用」不刷新）要头跟着改；终端头原来在包页画 `off` 那一项，现在那一项没了、换成「启用」。
