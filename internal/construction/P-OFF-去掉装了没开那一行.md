## 施工单 P-OFF：去掉「装了没开」那一行

状态：合了（2026-10-23）。项目主人看了一个会话 system 末尾的 `Installed but off in this session's preset: background, files, goal, history, knowledge, memory, miyu-memes, packages, peers, questions, script-roll_dice, show, subagents, todos, usage, web-fetch, web-search.` 问「这段内容是不是没必要」，在三个选项里选了整行删掉（主会话推荐的是只列平台、扩展包这类）。推翻设计 16 Y8。

### 定的

1. **新会话不再拼这一行**：`Snapshot::with_preset` 只记预设、去掉角色扮演提示，system 一字不加。只为这一行存在的「有工具的功能」（`snapshot::tooled`、`Parts.tooled`）、`CoreLines.preset_off`、资源 `core/preset-off.txt` 一起拿掉。
2. **以前造的快照照旧**：快照里存的是拼好的 system，那些会话照原样发，前缀一个字节不变。回合开始换快照的照新的拼（换快照本来就是改了预设、人格这些，前缀照例重新开始）。
3. **快照的 `preset.off` 照旧记全部没开的**：`preset.get`、设置页、`/why` 这些给人看的照旧照它。
4. **代价**：关了接入平台的预设里有人让她读群消息，她可能又拿 curl 去猜平台接口（旧版 issue #49）；实测出现了再重开。

### 做了什么

- `miyu-policy`：`with_preset(pin)`；`CoreLines` 去掉 `preset_off`。
- `miyu-store`：不再读 `core/preset-off.txt`；资源删掉。
- `miyu-session`：`Parts.tooled`、`snapshot::tooled` 删掉，开会话、换快照两处跟着去掉。
- 文档：设计 16 Y8（写明推翻和为什么）、08、18、26 第四节和登记簿那一行标删除；蓝图 presets.md、policy.md；prompts.md 重新生成。
- 测试：原来断言那一行在的几条改成断言它不在（会话层换预设、换快照、角色扮演关着；端点开会话、关了 MCP 服务、知识库那一块后面），风格锁照旧接在最后。
