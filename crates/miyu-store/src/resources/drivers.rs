//! 驱动写给模型的几句（`core/drivers/` 下，`docs/blueprint/policy.md`「`CoreTexts`」的 `drivers`）：施工 HT-2 从 `resources.rs`
//! 拆出来，那边到了行数上限。

use miyu_policy::{
    AttachedPathTexts, DriverPlaceholders, HostedToolTexts, ImageDescriptionTexts, ImageNameTexts,
    OpenPathTexts, PathOnlyTexts, TextFileTexts,
};

use super::{ResourceRoot, SourceError};

impl ResourceRoot {
    /// 驱动的占位，随核心附带的字的一部分（[`ResourceRoot::core_texts`]）。
    pub(super) fn driver_placeholders(&self) -> Result<DriverPlaceholders, SourceError> {
        let driver = |name: &str| self.read(&["core", "drivers", name]);
        Ok(DriverPlaceholders {
            image_omitted: driver("image-omitted.txt")?,
            file_omitted: driver("file-omitted.txt")?,
            no_output: driver("no-output.txt")?,
            tool_attachments: driver("tool-attachments.txt")?,
            tool_attachments_only: driver("tool-attachments-only.txt")?,
            text_file: Some(TextFileTexts {
                file_open: driver("file-open.txt")?,
                file_cut: driver("file-cut.txt")?,
                file_close: driver("file-close.txt")?,
            }),
            image_name: Some(ImageNameTexts {
                image_open: driver("image-open.txt")?,
                image_close: driver("image-close.txt")?,
                image_omitted_named: driver("image-omitted-named.txt")?,
            }),
            image_description: Some(ImageDescriptionTexts {
                image_description_open: driver("image-description-open.txt")?,
                image_description_open_named: driver("image-description-open-named.txt")?,
                image_description_close: driver("image-description-close.txt")?,
            }),
            attached_path: Some(AttachedPathTexts {
                image_omitted_path: driver("image-omitted-path.txt")?,
                file_omitted_path: driver("file-omitted-path.txt")?,
            }),
            open_path: Some(OpenPathTexts {
                image_open_path: driver("image-open-path.txt")?,
                file_open_path: driver("file-open-path.txt")?,
            }),
            path_only: Some(PathOnlyTexts {
                file_path_only: driver("file-path-only.txt")?,
                file_path_only_sized: driver("file-path-only-sized.txt")?,
            }),
            hosted_tool: Some(HostedToolTexts {
                hosted_tool: driver("hosted-tool.txt")?,
                hosted_tool_failed: driver("hosted-tool-failed.txt")?,
                hosted_tool_open: driver("hosted-tool-open.txt")?,
                hosted_tool_cut: driver("hosted-tool-cut.txt")?,
            }),
        })
    }
}
