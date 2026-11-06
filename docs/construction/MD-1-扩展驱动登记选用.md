## MD-1 扩展提供的模型驱动：登记和选用

状态：施工完，待 core1 审（2026-11-07）。设计 `66-扩展提供模型驱动.md` 第四、九、十、十三节（项目主人 2026-11-07 定挪到首个正式版以前做）。
配置那一格怎么认扩展的，开工时问了 core1，11-07 照推荐定 A（见「定了的」第 2 条）。MD-2 接通道以前，扩展提供的驱动一律当用不了。

### 定了的

1. 照设计：清单 `[[driver]]`（`name`、`cache`、`key`、`images`），只有扩展包能写，写了要声明能力 `model.driver`、声明了也要写；能力
   `model.driver` 排第二十，在 `search` 后面；确认屏列出驱动名；`enum Driver` 加扩展的一格；`miyu check` 报装没装、批没批、驱动名对不对、
   key 合不合；档案、npm 表、默认都不指向扩展。
2. **配置那一格（A，core1 11-07）**：`providers.<id>.driver` 的选项照 N-5 搜索来源的做法加长（内置三种，加上装着、声明了 `[[driver]]`
   的 `<包>/<驱动名>`）；只对这一格、只对 `<包编号>/<驱动名>` 这个写法开窄口子：不在选项里的照原样留，记警告 `unknown_driver`，这一家用不了、
   **绝不退回内置驱动**；别的不合法写法照旧 `not_an_option`。`config.set` 同样收；读回、`miyu check`、头的下拉框（列不到时显示原值）一致；
   改回内置名、删掉照常。测试里加「没装的扩展驱动绝不退回内置驱动发请求」：假端点一个请求都没收到。

### 做了什么

- **清单**（`miyu-config/src/package/driver.rs`）：`DriverDecl {name, cache, key, images, line}`，`DriverCache`、`DriverKey`；驱动名照命令名的
  写法、一个包里不重；`cache` 默认 `best_effort`、`key` 默认 `optional`、`images` 默认假；`check` 查和能力两头对得上。新的出错代码
  `bad_driver`（三种语言）；字段白名单 `DRIVER`、表数组的白名单、扩展包能写的表都加了 `driver`；作者文档 `manifest`（中英）加
  `[[driver]]` 一节和能力表一行。
- **能力**：`Capability::ModelDriver`（`model.driver`），排在 `Search` 后面，`ALL` 二十项；名字、一句说明（三种语言）「提供模型驱动：用它的
  模型时，发给模型的全部对话都经过它」。不附带网络。
- **配置的窄口子**（`miyu-config/src/driver_ref.rs`）：`PROVIDER_DRIVER`、`extension`（写法）、`check`（读、`config.set` 都走它：合的照常，
  是那个口子的值照留、交回警告码）、`with_installed`（选项接上装着的）。读配置（`parse.rs`）照留时记一条 `unknown_driver`，`name` 是写的值；
  新的配置问题代码 `UnknownDriver`（警告，三种语言）。核心起来时（`miyu-core` 的 `Packaged::all`）照装着的包接上选项，下拉的名字照
  「包的名字：驱动名」（`miyu-store` 的 `human.rs`）。
- **驱动**（`miyu-models/src/provider.rs`）：`Driver::Extension(DriverRef {package, name})`，不再是 `Copy`；`parse` 认 `<包>/<驱动名>`，
  新的 `builtin` 只认内置的；`provider()` 手写的照 `parse`，档案、npm 表、默认照 `builtin`；模型的包名换驱动也照 `builtin`；
  `build` 交 `Option`（扩展的没有）；`switchable` 扩展的为假；温度上限扩展的照清单本身的 2。`Driver` 大了（带两个字符串），`reference.rs` 的
  `Resolved::Model` 改装 `Box<Target>`，免得和池差得太多。目录的「支持不支持」照 `builtin`。
- **运行时**（`miyu-session`）：`Routes::choices` 开头认出扩展的就 `no_model`（`route::unbuilt` 那一句），单个模型当场报、池里跳过，不取它的
  key、地址；`ready`、列模型、`provider.test`、远程 embedding 造不出驱动的同样当用不了；本机服务照 `builtin`。
- **`miyu check`**（`miyu-endpoint/src/check/drivers.rs`）：装着、驱动名对的那几家，包没批 `model.driver` 报 `driver_unapproved`（错误），
  `key = "required"` 没配 key 报 `driver_key_missing`（错误），`key = "none"` 配了 key 报 `driver_key_unused`（警告）。没装、写错名字的由
  配置那一层报 `unknown_driver`。三句话三种语言。
- **确认屏**（`packages/preview.rs`、命令行 `pkg/confirm.rs`）：`provides.drivers` 列驱动名，命令行印「模型驱动 <名字>」。
- **文档**：设计 66 第四节第 5 条写明 A；蓝图 `extensions.md`、`packages.md`（二十个、`bad_driver`）、`protocol.md`、`config.md`
  （`providers.<id>.driver` 一行、`unknown_driver`）、`models.md`（只认手写的、当用不了）；参考 `extensions.md`（中英）能力表；作者文档。

### 测试

- `miyu-config`：`driver/tests.rs`（读成四格、默认值；写错的、重了、少了名字、写成 `[driver]`、不认识的格、两头对不上、不是扩展包的拒）；
  `driver_ref/tests.rs`（写法；装着的照常；没装的照留、记警告、`name` 是写的；别的写法、别的格照旧 `not_an_option`）；能力顺序。
- `miyu-models`：`provider/tests/extension.rs` 的 `only_a_hand_written_driver_points_to_an_extension`：手写的是扩展的、造不出、没开关、模型的包名压不过它；档案写成扩展的样子
  这一家用不了；默认是内置的；npm 表换成扩展的样子这个模型用不了；`builtin` 不认扩展的。
- `miyu-core`：`an_extension_driver_joins_the_provider_dropdown`（选项接在内置的后面、配置读得进、名字「Gravity: ag」）。
- `miyu-session`：`route_extension_driver.rs`：写了装着的样子、没装的两种，单个模型当场 `no_model`、原话对，**假端点一个请求都没收到**（配了 key
  也没去取）；池里扩展那一家跳过、换下一个，那一家的假端点一个请求都没收到。
- `miyu-endpoint`：`extension_driver.rs`：`miyu check` 报四条（`gravity` 没批、它要 key 没配，`plainer` 在开关文件里批过、不报没批，它不要 key 却配了，一个没装）；下拉列出装着的、
  名字对；`config.set` 收没装的、装着的、内置的，别的写法拒，读回来记警告；`package.info` 的 `provides.drivers`、能力 `model.driver`。
- `miyu-cli`：确认屏「包含：扩展程序、模型驱动 ag」（中英）。

### 手搓的变异

14 个，全红：包里写了 `[[driver]]` 却不要 `model.driver`、要了却没写，这两种都放过（`driver/tests.rs`）；口子开给每一格、开给每一种写法、
照留却不记警告（`driver_ref/tests.rs`）；档案、npm 表也能指向扩展的（`provider::tests`）；`choices` 放扩展的过去（`route_extension_driver`：
退回内置的发了请求）；`config.set` 用不带口子的检查；检查当成都没批、当成都批过，key 的两条规矩对调；确认屏不列驱动；下拉框不接装着的
（`extension_driver`、`settings::tests`）。头一轮「当成都没批」那一个没红：当时测试里两个包都没批，分不出来；补上 `plainer` 在开关文件里
批过以后，两个方向都红。
