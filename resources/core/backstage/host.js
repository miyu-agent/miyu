// 核心按需起的后台页服务的宿主页（施工 BP-3，docs/blueprint/package-pages.md「不靠网页打开软件后台」第 3 条）：照网页「框和网页之间怎么说」那一套。
// 框第一次载入完交一条 MessageChannel（只交这一次）；框的请求原样 POST 给 /r/<票>，核心照票认包、只答那四样；/e/<票> 推来的照原样转进框。
// 深浅照系统：`context` 补上 `theme`，系统换了深浅推 `theme.changed`。不带颜色。

const ticket = location.pathname.split('/')[2];
const frame = /** @type {HTMLIFrameElement} */ (document.getElementById('page'));
const media = matchMedia('(prefers-color-scheme: dark)');
const theme = () => (media.matches ? 'dark' : 'light');
/** @type {MessagePort|null} */
let port = null;

/** 一条请求交给核心，交回整条回应。 @param {any} msg */
async function rpc(msg) {
  const res = await fetch(`/r/${ticket}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(msg) });
  return res.json();
}

/** 框发来的一条。 @param {any} msg */
async function handle(msg) {
  if (!port) return;
  if (!msg || typeof msg !== 'object' || msg.jsonrpc !== '2.0' || typeof msg.method !== 'string') {
    port.postMessage({ jsonrpc: '2.0', id: msg?.id ?? null, error: { code: -32600, message: 'Invalid Request' } });
    return;
  }
  let reply;
  try {
    reply = await rpc(msg);
  } catch (err) {
    reply = { jsonrpc: '2.0', id: msg.id, error: { code: -32603, message: String(err) } };
  }
  if (msg.method === 'context' && reply?.result) reply.result.theme = theme();
  if (msg.id !== undefined && msg.id !== null) port.postMessage(reply);
}

const context = await rpc({ jsonrpc: '2.0', id: 0, method: 'context' });
const id = context?.result?.package;
document.documentElement.lang = context?.result?.language ?? 'en';
if (id) document.title = id;

frame.addEventListener('load', () => {
  if (port || !frame.getAttribute('src')) return;
  const channel = new MessageChannel();
  port = channel.port1;
  port.onmessage = (e) => { handle(e.data); };
  frame.contentWindow?.postMessage({ miyu: 'port' }, '*', [channel.port2]);
  const events = new EventSource(`/e/${ticket}`);
  events.onmessage = (e) => {
    try {
      port?.postMessage(JSON.parse(e.data));
    } catch {
      // 读不懂的一条不转
    }
  };
});
media.addEventListener('change', () => {
  port?.postMessage({ jsonrpc: '2.0', method: 'theme.changed', params: { theme: theme() } });
});
if (id) frame.src = `/p/${ticket}/${encodeURIComponent(id)}/`;
