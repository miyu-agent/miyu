//! 项目的附加提示词（施工 H-3，设计 `38-项目.md` 第四节）：回合开始照会话现在在哪个项目，读 `projects.json` 找那个项目的
//! `prompt`，照模板拼成一块（`miyu_policy::project_block`）。项目没了、没写提示词的没有。派出去的子代理进不了项目，自然不带；
//! 分叉出来的带着父会话的历史，跟着父会话在的项目带（前缀和父会话逐字节一样，缓存照用）。

use super::Refresh;

/// 会话现在在 `project` 里：这一刻该放进 system 的那一块；不在项目里、项目没了、没写的没有。
///
/// # Errors
///
/// 项目文件读不成、模板读不了：英文一句，照旧用快照里的。
pub(crate) fn block(refresh: &Refresh, project: Option<&str>) -> Result<Option<String>, String> {
    let Some(project) = project else {
        return Ok(None);
    };
    let read = miyu_store::projects::read(&refresh.projects).map_err(|error| error.to_string())?;
    let Some(prompt) = read
        .projects
        .find(project)
        .and_then(|found| found.prompt.clone())
    else {
        return Ok(None);
    };
    let template = refresh
        .resources
        .project()
        .map_err(|error| error.to_string())?;
    Ok(miyu_policy::project_block(&template, &prompt))
}
