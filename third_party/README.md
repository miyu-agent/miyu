# 补过的第三方库

这里放的是从 crates.io 原样拷来、再补过几处的第三方库。用到它的 crate 写成路径依赖指到这里（施工 MP-1：原来用根目录的 `[patch.crates-io]`，那只在本工作区里管用，钉了核心的头、扩展编不过），根目录的 `[workspace]
exclude` 把它们排除在工作区外（路径依赖照样会被 `cargo fmt --all` 查到，所以每个目录里放一份 `disable_all_formatting = true` 的 `rustfmt.toml`）：它们不是我们的代码，门禁的 clippy、文档、行数这些规矩不管它们；许可证照包清单里的 `license`
照常查（`docs/blueprint/licenses.md`）。

每补一处都写在下面：改了哪个文件、为什么、怎么验。库升级时，先拷新版原样替换，再照这里把补丁一处处挪过去，对着下面的
「怎么验」重验一遍。

## mermaid-rs-renderer 0.3.1（MIT，https://github.com/1jehuang/mermaid-rs-renderer）

施工 V-35（`docs/construction/V-35-画图字体不整份进内存.md`），2026-11-02 项目主人定：只在我们这里补。

拷了 `src/`、`LICENSE`、`README.md`、`Cargo.toml`；`tests/`、`benches/`、`examples/` 没拷，`Cargo.toml` 里相应的 `[[test]]`、
`[[bench]]`、`[[example]]` 和开发依赖 `criterion` 跟着删掉。原版自带的单元测试 `include_str!` 了发布包里没有的 `docs/` 文件，
从 crates.io 下下来本来就编不过，不跑。

改了的只有 `src/text_metrics.rs`（量字宽）和 `src/lib.rs`（多导出两个函数），`Cargo.toml` 多一个依赖 `memmap2`：

1. 字体文件映射进来（`FontData::Mapped`），不整份拷进内存。原版 `data.to_vec()`：中文字体一个文件十几 MB，画过一张图就一直
   常驻。字体不在文件里的（`fontdb::Source::Binary`）照旧拷一份。
2. 扫系统字体库用的整张表只在找字体的那一会儿留着（原版常驻在 `TextMeasurer` 里）。
3. 缓存只记字体在哪个文件的第几张、文件多长、什么时候改的（`<哈希>.location`，几十字节），不再另存一份整的字体（原版
   `<哈希>.font`）；记下的对得上就直接映射，不扫系统字体库；对不上就照旧扫、重记。缓存的键换了版本（`v3-font-location`）。
4. 记位置的目录由调用方定（`set_font_cache_dir`，`None` 不记）；没定过的照原版 `$XDG_CACHE_HOME/mmdr/font-cache`。那个默认
   目录是几个用这个库的程序共用的，里面原版存的整份字体分不清是谁的，一律不碰。
5. `has_remembered_font`：问一串字体名记没记过、那个文件变没变，`miyu-mermaid` 探字体时先问它，记过的不用再扫系统字体库。

怎么验：同一批图（中文、英文、带重音的拉丁字母、emoji、时序图、类图、状态图、甘特图、饼图、三十个节点的长图）补前补后画出来的
SVG 逐字节一样，头一回和照记下的位置画的那一回都比；`crates/miyu/tests/mermaid_fonts.rs` 守着只记位置、不写默认目录、重起以后
画的一样；数字见施工单。
