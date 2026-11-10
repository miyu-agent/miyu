//! 照名字答的几种请求（施工 F-6 中、N-5、X-14 下、BUS-2）：`serve` 读到 `method.call`、`search.query`、`command.call` 时照方法名、
//! 查的字、命令跟的字定怎么答，一行 JSON-RPC 回应；不答的交 `None`。

/// 答编号是 `id`、方法是 `method` 的那一次 `method.call`（施工 F-6 中）；`slow` 不答，等核心到时限；`give` 交回 `give` 那几段字
/// （施工 W-6 三补）；`huge` 回一行超过核心一行上限（1 MiB）的结果（施工 YC-5补）。
pub(crate) fn answered(id: &str, method: &str, give: &str) -> Option<String> {
    Some(match method {
        "slow" => return None,
        "give" => {
            let given: Vec<String> = give.split(',').map(|one| format!("{one:?}")).collect();
            format!(
                r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"given":[{}]}}}}"#,
                given.join(",")
            )
        }
        "boom" => {
            format!(r#"{{"jsonrpc":"2.0","id":"{id}","error":{{"code":-32001,"message":"boom"}}}}"#)
        }
        "huge" => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"pad":"{}"}}}}"#,
            "x".repeat(1024 * 1024 + 4096)
        ),
        // 带字的码（施工 BUS-2）：照 JSON-RPC 的写法放在 `error.data.code`。
        coded if coded.ends_with(".coded") => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","error":{{"code":-32000,"message":"coded","data":{{"code":"bad_audio"}}}}}}"#
        ),
        _ => format!(r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"answered":"{method}"}}}}"#),
    })
}

/// 答编号是 `id`、查的是 `query` 的那一次 `search.query`（施工 N-5）。
pub(crate) fn searched(id: &str, query: &str) -> String {
    match query {
        "limit" => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","error":{{"code":-32000,"message":"quota used up","data":{{"reason":"limited"}}}}}}"#
        ),
        "boom" => {
            format!(r#"{{"jsonrpc":"2.0","id":"{id}","error":{{"code":-32000,"message":"boom"}}}}"#)
        }
        _ => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"hits":[{{"title":"found {query}","url":"https://ext.example/{query}"}}]}}}}"#
        ),
    }
}

/// 答编号是 `id`、命令是 `command`、跟的字是 `argument` 的那一次 `command.call`（施工 X-14 下）。
pub(crate) fn commanded(id: &str, command: &str, argument: &str) -> Option<String> {
    Some(match argument {
        "silent" => return None,
        "boom" => {
            format!(r#"{{"jsonrpc":"2.0","id":"{id}","error":{{"code":-32000,"message":"boom"}}}}"#)
        }
        "garbled" => format!(r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"nothing":1}}}}"#),
        _ => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"said":"{command} {argument}\nok"}}}}"#
        ),
    })
}
