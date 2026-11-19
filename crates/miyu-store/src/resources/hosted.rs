//! 供应商侧工具的说明（施工 DSR-4补，设计 `72-DeepSeek内置联网工具.md` 第二节第 2 条）：`core/hosted/<工具名>.txt`，一件
//! 一份，换成这件工具的会话拼进 system（`miyu_policy::Snapshot::with_provider_search`）。哪几件有说明是数据：加一件只加
//! 一份文件。

use std::io;

use super::{ResourceRoot, SourceError};

impl ResourceRoot {
    /// 供应商侧工具 `name` 的说明；没有这一份文件的没有。`name` 是供应商档案里写的名字，只认字母、数字、`_`、`-` 拼成的：
    /// 别的写法（带路径分隔符的）当没有，不拿去拼路径。
    ///
    /// # Errors
    ///
    /// 有这一份文件却读不了，写明是哪一份。
    pub fn hosted_note(&self, name: &str) -> Result<Option<String>, SourceError> {
        let plain = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-';
        if name.is_empty() || !name.bytes().all(plain) {
            return Ok(None);
        }
        match self.read(&["core", "hosted", &format!("{name}.txt")]) {
            Ok(text) => Ok(Some(text)),
            Err(SourceError::Read { error, .. }) if error.kind() == io::ErrorKind::NotFound => {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::ResourceRoot;

    #[test]
    fn a_hosted_tool_has_a_note_only_when_its_file_exists() {
        let shipped =
            ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
        let note = shipped.hosted_note("deepseek_web_search").expect("读得了");
        assert!(
            note.expect("有这一份")
                .starts_with("Web search is `deepseek_web_search`")
        );
        assert_eq!(
            shipped.hosted_note("no_such_tool").expect("没有不算错"),
            None
        );
        // 带路径分隔符、空的名字不拿去拼路径。
        for odd in ["", "../core/machine", "a/b", "a.b", "..\\x"] {
            assert_eq!(shipped.hosted_note(odd).expect("不算错"), None, "{odd}");
        }
    }
}
