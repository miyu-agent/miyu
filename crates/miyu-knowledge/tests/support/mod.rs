//! 知识库几件工具的测试共用的（施工 K-3 从 `tools.rs` 挪出来守 500 行上限）：一个假端口，照工具名调一次，读出交回的字、
//! 给人看的说法。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::block::Block;
use miyu_kernel::time::UtcOffset;
use miyu_tool::{
    Call, Done, KbHit, KbPage, KbRefused, KbSave, KbTrashed, KbWritten, KnowledgePort, Pending,
    Progress, Stop, Tool,
};

/// 一个假端口：记下她要做的，照预先定好的交回。
#[derive(Default)]
pub struct Fake {
    pub searched: Mutex<Vec<(String, Option<String>)>>,
    pub read: Mutex<Vec<(String, String, usize)>>,
    pub saved: Mutex<Vec<KbSave>>,
    pub edited: Mutex<Vec<(String, String, String, String)>>,
    pub deleted: Mutex<Vec<(String, String)>>,
    pub refuse: Option<KbRefused>,
    pub hits: Vec<KbHit>,
    pub page: Option<KbPage>,
    pub written: Option<KbWritten>,
}

impl KnowledgePort for Fake {
    fn search<'a>(
        &'a self,
        query: String,
        kb: Option<String>,
    ) -> Pending<'a, Result<Vec<KbHit>, KbRefused>> {
        self.searched
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((query, kb));
        let answer = self.refuse.clone().map_or(Ok(self.hits.clone()), Err);
        Box::pin(async move { answer })
    }

    fn read<'a>(
        &'a self,
        kb: String,
        path: String,
        page: usize,
    ) -> Pending<'a, Result<KbPage, KbRefused>> {
        self.read
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((kb, path, page));
        let answer = match &self.refuse {
            Some(refused) => Err(refused.clone()),
            None => Ok(self.page.clone().expect("备了一页")),
        };
        Box::pin(async move { answer })
    }

    fn save<'a>(&'a self, save: KbSave) -> Pending<'a, Result<KbWritten, KbRefused>> {
        self.saved
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(save);
        let answer = self.written();
        Box::pin(async move { answer })
    }

    fn edit<'a>(
        &'a self,
        kb: String,
        path: String,
        old: String,
        new: String,
    ) -> Pending<'a, Result<KbWritten, KbRefused>> {
        self.edited
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((kb, path, old, new));
        let answer = self.written();
        Box::pin(async move { answer })
    }

    fn delete<'a>(&'a self, kb: String, path: String) -> Pending<'a, Result<KbTrashed, KbRefused>> {
        self.deleted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((kb, path));
        let answer = self.refuse.clone().map_or(
            Ok(KbTrashed {
                path: PathBuf::from("/kb/notes/old.md"),
                trash: "/trash/old.md".to_string(),
            }),
            Err,
        );
        Box::pin(async move { answer })
    }
}

impl Fake {
    fn written(&self) -> Result<KbWritten, KbRefused> {
        match &self.refuse {
            Some(refused) => Err(refused.clone()),
            None => Ok(self.written.clone().expect("备了写进去的")),
        }
    }
}

pub fn written(before: Option<&str>, after: &str, created: bool) -> KbWritten {
    KbWritten {
        path: PathBuf::from("/kb/notes/a.md"),
        before: before.map(|text| text.as_bytes().to_vec()),
        after: after.as_bytes().to_vec(),
        created,
    }
}

pub fn tools() -> Vec<Arc<dyn Tool>> {
    miyu_knowledge::tools(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
        .expect("读得出")
}

/// 用假端口 `port`（没有的是这个会话查不了知识库）调一次 `name`。
pub async fn call(port: Option<Arc<Fake>>, name: &str, args: serde_json::Value) -> Done {
    let tool = tools()
        .into_iter()
        .find(|tool| tool.spec().name == name)
        .expect("有这件");
    let call = Call {
        args: args.to_string(),
        cwd: String::new(),
        home: None,
        data_root: None,
        seen: Arc::default(),
        stop: Stop::default(),
        sandbox: None,
        log: None,
        offset: UtcOffset::from_minutes(0).expect("零时区"),
        agents: None,
        messages: None,
        jobs: None,
        sessions: None,
        usage: None,
        questions: None,
        memory: None,
        knowledge: port.map(|port| port as Arc<dyn KnowledgePort>),
        goals: None,
        nested: None,
        packages: None,
        ids: None,
        settings: None,
    };
    tool.run(call, Progress::new(|_| {})).await
}

pub fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

pub fn said(done: &Done) -> String {
    done.human
        .as_ref()
        .map(|said| said.key.clone())
        .unwrap_or_default()
}

pub fn hit(kb: &str, path: &str, heading: &str, text: &str) -> KbHit {
    KbHit {
        kb: kb.to_string(),
        path: path.to_string(),
        heading: heading.to_string(),
        page: None,
        text: text.to_string(),
    }
}
