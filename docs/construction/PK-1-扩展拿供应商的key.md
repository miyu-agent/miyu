## PK-1 扩展拿主人配的某一家供应商的 key

状态：施工完，待 core1 审（2026-11-14）。来源：项目主人 2026-11-14 定做官方包 `deepseek`（DeepSeek 的 TTS、ASR 插语音引擎，OCR
做工具），key 不另填，用主人在模型那边配好的 DeepSeek key；core1 定 A：加一项新能力，装时人批。蓝图 `extensions.md`（能力、出厂的
例外、「配置」第 5 条、守着它的测试）、`packages.md`（能力个数、出厂的例外、`bad_provider_key`）、`protocol.md`（出厂的例外）；
设计 05 的能力表；协议参考 `extensions`（中英：握手、能力表、`provider_key` 一节）；作者文档 `manifest`（中英：能力表、
`[provider_key]` 一节）、`extensions`（中英：「握手和配置」）。

### 目的

包读不到主人配的模型 key：握手只交包自己的配置项；`config.get` 给 `providers.<id>.key` 交的是引用 `{"secret": "<名字>"}`；
`secret.list` 不交值。要一条窄路：包在清单里说要哪一家，人在装的那一屏看清「这个包会拿到你 DeepSeek 的 key」批了，核心才把那一家
的 key 交给它，别家的一律不交。

### 做了什么

- **能力 `provider.key`**（`miyu-config/src/package/capability.rs`）：第二十三项，排在 `model.driver` 后面（同是模型那一块的）。
  `Capability::needs_person` 认它：出厂的包声明了也不算批过（同 `login.page`）。名字「使用模型供应商的 key」，说明两句：知道是哪
  一家的「这个包会拿到你 {provider} 的 key」（`extensions/capabilities/provider.key/named`），不知道的（以前生成的索引）「会拿到你在
  模型那边配好的某一家供应商的 key」（`…/summary`），英、日对应，进 `resources/core/human` 三份。
- **清单 `[provider_key] catalog = "<目录里的编号>"`**（`package/provider_key.rs`）：只有扩展包能写，`catalog` 照配置里
  `providers.<id>.catalog` 的写法（`secret::valid_name`）。写了表要声明能力，声明了能力要写表（同 `[[driver]]` 和 `model.driver`），
  对不上、少了、写法不对的报新代码 `bad_provider_key`；别的格 `unknown_key`。`Manifest::provider_key`。为了 `package.rs` 不超
  500 行，`Mascot` 挪进了读它的 `look.rs`（对外照旧是 `package::Mascot`）。
- **认法抽到一处**（`miyu-models/src/provider_key.rs` 的 `serving(values, catalog)`）：原来 `miyu-net` 的 `deepseek_provider` 挪
  过来做成通用的，联网包的 DeepSeek 搜索和这一步共用。`providers.<id>` 算那一家：写了 `catalog` 的照它；没写的看字面地址里有没有
  官方地址（`OFFICIAL` 表，现在只有 `deepseek → api.deepseek.com`）；两样都没写的看编号。照编号排第一家，没写 key 的不算，地址是
  环境变量引用的不算。`miyu-net` 多依赖 `miyu-models`（第 3 层依赖第 2 层）。
- **交法**（`miyu-endpoint/src/extensions/provider_key.rs`、`config.rs` 的 `given`）：握手（`connection/handing.rs`）和
  `extension.config` 推送（`subscriptions/extension_config.rs`）都改照 `given` 算：包自己的配置项，再加一格 `provider_key`，值是
  认出来的那一家的 key 解成的真值（照包自己的密钥项那一路，`Config::secret`）。没写 `[provider_key]`、没批、没配那一家、key 取不到
  的不放这一格。推送照原来的规矩跟上一次比：换了 key 推新值，删了那一家、去掉了 key 推 `null`。`config.get` 不动，照旧只交引用。
- **装包那一屏**（`extensions/approval.rs` 的 `named` 多一个 `catalog`）：有清单的（`package.install` 看一眼、`extension.status`、
  `package.info` 装着的）照清单，软件源装的照索引条目新加的 `provider_key`（`miyu-source` 的 `Entry`，`collect.rs` 照清单写，
  旧索引没有、照不说是哪一家的那一句）。显示名照目录（`provider_side(catalog, None, Some(catalog)).name`，DeepSeek 是
  「DeepSeek」），目录还没读完、没有这一家的照编号。命令行确认屏本来就印「名字 说明」，不用改。

### 交给扩展的形状（语音会话照这个写包）

清单：

```toml
[process]
capabilities = ["provider.key"]

[provider_key]
catalog = "deepseek"
```

握手 `hello` 的回应 `config`（和包自己的键放在一起）：`{"provider_key": "sk-…", "<前缀>.<名字>": …}`；没有那一家时没有这一格。
之后 `extension.config {"keys": {"provider_key": "sk-新的…"}}`、`{"keys": {"provider_key": null}}`。

### 定的技术题

1. **格子的名字 `provider_key`、值是字符串**：包自己的配置项一律是 `<前缀>.<名字>`，不带 `.` 的这一格撞不上；跟清单的表名、能力名
   一个说法。只交 key 本身：包自己声明了要哪一家，不用再告诉它；地址也不交（官方包照官方地址连，主人配的是中转时另说）。
2. **清单字段 `[provider_key] catalog`**：`catalog` 和配置里 `providers.<id>.catalog` 同一个词、同一套编号、同一个写法，作者照
   `provider.catalog` 搜得到的编号写。一个包只要一家（一张表，不是数组）：现在只有 DeepSeek 一个用法，要几家的以后再改成数组。
3. **能力排在 `model.driver` 后面**：同是模型那一块的；编号接在现有表后面，是第二十三项。
4. **认法多一条「都没写的看编号」**：第一次接入配 DeepSeek 写的就是 `providers.deepseek.key`，不写 `catalog`、不写地址
   （`miyu-cli` 的 `setup/flow.rs`：编号和目录编号一样时不写 `catalog`），核心照编号找档案。原来的 `deepseek_provider` 认不出这种，
   联网包的 DeepSeek 搜索这一步起也认得出了。
5. **写了别家 `catalog` 的，地址再像也不算**：原来是「地址像 或者 `catalog` 是它」两条或起来；改成写了 `catalog` 的只认 `catalog`。
   交 key 给第三方程序，主人明说了是别家的就不猜。只影响「`catalog` 写别家、地址却是官方地址」这种自相矛盾的配置。
6. **官方地址表放在 `miyu-models`，不读档案、目录**：交 key 在每次配置变了的推送里算，不该等目录；联网包手里也没有档案。只放实测
   过的一家，以后加一行。
7. **交 key 不复用 MD-2 的那一路**：MD-2 是会话路由挑到扩展驱动那一家时、每次调用照那一家的 `key` 取，跟这里「照清单认一家、跟着
   配置推」不是一回事；能复用的是取真值那一步，跟包自己的密钥项同一个 `Config::secret`。
8. **核心交之前再查一遍批没批**：没批全的包本来就不拉起，这一道是防以后拉起的条件改松；读开关文件只在清单写了 `[provider_key]`
   的包上做。

### 拿不准的（报告里同样列给 core1）

1. 编号 `PK-1` 以前在代码注释里用过一次（`miyu-config` 的 `kinds.rs`、`package/tests.rs`、蓝图 `packages.md`：「只带一条子命令
   的 `bundle`（施工 PK-1：迁移程序）」），施工单目录、README 里没有那一步。这一步照派工单叫 PK-1，旧注释没动。
2. 第 4 条让联网包的 DeepSeek 搜索多认得出一种配置（只写了 `providers.deepseek.key` 的），算行为变化，但和设计 36 第二节第 1 条
   「用模型那边配了 DeepSeek 的那一家的 key」一致。

### 怎么验

- `miyu-endpoint` 的 `tests/extension_provider_key.rs`：批过的握手拿到 DeepSeek 那一家的真值（配置里另有 OpenRouter）；换了别家的
  key 不推、换了这一家的推新值、去掉这一家的 key 推 `null`；整份记录里搜不到别家的 key；`config.get` 照旧是引用。只配了别家的，
  握手的 `config` 是空的。没批的打开回 `needs_approval`（`data.capabilities` 是 `["provider.key"]`），不拉起、拿不到。
- `tests/extension_approval.rs`：出厂的夹具扩展多声明 `provider.key`，`unapproved` 只有它，说明里写着那一家。
- 单测：`miyu-models` 的 `provider_key/tests.rs`（三种认法；别家的 `catalog`、不是官方的地址、环境变量地址、没写 key 的不算；几家照
  编号；没配那一家的没有）；`miyu-config` 的 `provider_key/tests.rs`（读成编号；两头对不上、少了、不是字、写法不对、别的格、写成数组、
  不是扩展包）、能力表的先后和要人批的两项；`fields` 的「每一格都写进了中英 `manifest`」；`miyu-net` 照旧（借 DeepSeek 的 key）。
- 改前红：见提交说明。
