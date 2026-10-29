## 施工单 PT-2 外：别的目录里的技能改成可配置

状态：写好了（2026-10-29）。PT-2 中起 Miyu 照家目录无条件读 `~/.agents/skills/`。项目主人 2026-10-29 改主意（网页会话转来）：不撤，改成一个配置项「读取其他目录的 Skills」：一个总开关（出厂关），一张能增删的目录表（出厂一项 `~/.agents/skills`），网页放在技能页顶上。形状由主会话定（2026-10-29）。图纸 `docs/blueprint/skills.md`、`config.md`。

### 目的

人自己决定读不读别的客户端装的技能、读哪几个目录；关着的时候 Miyu 一个都不碰，请求和没有这些目录时逐字节一样。

### 定了的

1. 两项配置，只写在 Miyu 的配置里，项目配置不收，`next_turn`（照技能本来的时机：回合开始再找一遍，列表变了走尾巴，system、工具不动）：`skills.external.enabled`（开关，`false`）、`skills.external.dirs`（文字的列表，`["~/.agents/skills"]`）。设置页「高级」那一页新开「技能」一组，不设 `hidden`：终端、命令行也看得到；网页按键名取出来画在技能页顶上，通用设置页按前缀滤掉。
2. 目录写成 `~`、`~/…` 或绝对路径，`~` 照现有的家目录规则拼（`miyu_fs::written`）。相对路径 `config.set` 时拒，手改的文件 `miyu check`、`config.check` 也报 `bad_format`（别的项手改的只在读的时候算，这一项例外：读的时候认不了的静默跳过，人看不出来少了）。读不到的目录静默跳过，记一行 `DEBUG`、不写路径。
3. 先后：自己放的赢，其次包带的，再其次别的目录、几个之间照表里的先后；被压掉的不记、不报，`skill.list` 里也不列。
4. `skill.list` 的 `from` 照旧是 `agents`（`skill.list` 在协议 1 的冻结表里，已有的值不改名；它指所有别的目录，不只 `~/.agents`），另加 `dir`：配置里写的那个目录原样。删照旧拒 `skill_shared`，说的话不再点名 `~/.agents/skills`。改了开关、目录表不另推：头照 `config.changed` 的 `keys` 重读 `skill.list`。
5. 出厂的非空列表：`miyu-config` 的 `Value::List` 改成 `Cow<'static, [Value]>`，同 `Value::Text`，清单里不可变的出厂值借代码里写的切片；以后别的项要非空的出厂列表也照它写。
6. `DataRoot` 不再记 `shared_skills`（PT-2 中加的那一格拿掉）；`miyu_store::skills::find` 多交一个别的目录的表，调的几处照这一刻（会话里照这一轮）的配置交，关着的交空的。

### 不做什么

- 不默认认 `~/.claude/skills/`、项目目录里的 `.agents/skills/`：要的自己写进目录表。
- 不新加推送、不新加错误码（复用 `bad_format` 和私密目录写法不对的那一句）。

### 验收

1. `miyu-store`：别的目录排在自己的、包的后面，几个之间照先后，被压掉的不记，没交的不碰；配置关着、没写的一个都不交，开着的照表、`~` 照家目录拼、相对路径和没有家目录的 `~` 跳过、重了的留前一个。
2. `miyu-endpoint`：`skill.list` 关着不列、开着列出 `from: agents` 和 `dir`、表里先列的赢；`skill.remove` 拒 `skill_shared`；`config.set` 拒相对路径、`config.check` 报手改的、`config.schema` 的出厂值；家目录下放着 `.agents/skills` 而开关关着的，造的会话 system、工具和没放时逐字节一样，会话中途开了的下一轮走尾巴、system 和工具不动，开着造的列进 system。
3. 配置清单的先后、三种语言的名字、生成的样本（`MIYU_SAMPLE_WRITE=1` 重生），`miyu config get` 的整段输出。
4. 手写变异；`cargo xtask check`；Windows 版 clippy；三平台 CI。
