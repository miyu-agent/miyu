//! 头读后台命令的输出（施工 7-4 补，协议的 `job.output`），和等它长了的信号（施工 V-6 上，`job.output` 的 `wait`）。

use tokio::sync::{oneshot, watch};

use miyu_kernel::id::JobId;
use miyu_tool::Output;

use super::{Handle, Message, Stopped};
use crate::jobs::Unreadable;

impl Handle {
    /// 头读后台命令 `job` 的输出（施工 7-4 补，协议的 `job.output`）：交回读得到的字、它还在不在跑。读的和她用 `jobs` 读的
    /// 是同一份（`docs/blueprint/session/tools.md` 第 6 条第 3 款）：结束了、存成 blob 的读 blob，别的读会话目录下的输出
    /// 文件，跑着的读到这时的。不进内核、不写盘：是什么照名册当场看，开文件另起一个任务，不占 actor。
    ///
    /// # Errors
    ///
    /// 会话停了。里面那一层：这个会话没派过这个任务（[`Unreadable::Unknown`]），或者它是子代理（[`Unreadable::Agent`]）。
    pub async fn job_output(&self, job: JobId) -> Result<Result<Output, Unreadable>, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Output { job, reply })?;
        answer.await.map_err(|_| Stopped)
    }

    /// 这个会话的任务那一层的信号（施工 V-6 上）：哪一条后台命令的输出写进了一段、哪一条结束落了盘都变一次。头带 `wait`
    /// 读 `job.output` 的先拿着它再读，没有新的就等它变，不隔一阵再问。会话停了，信号跟着断。
    ///
    /// # Errors
    ///
    /// 会话停了。
    pub async fn job_changes(&self) -> Result<watch::Receiver<u64>, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Changes(reply))?;
        answer.await.map_err(|_| Stopped)
    }
}
