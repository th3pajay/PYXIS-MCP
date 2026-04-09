use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{delete, get, post, put},
    Json, Router,
};
use futures_util::{stream::Stream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

use crate::{
    pyxis_client::PyxisClient,
    chatbot::{Chatbot, Message},
    llm_engine::LlmEngine,
    models::MoveStatus,
    vector_search::VectorIndex,
};

#[derive(Clone, Serialize, Default)]
pub struct DownloadProgress {
    pub pct: u8,
    pub bytes_done: u64,
    pub total: u64,
    pub done: bool,
    pub error: Option<String>,
}

pub struct GuiState {
    pyxis: Arc<PyxisClient>,
    index: Arc<Mutex<VectorIndex>>,
    pyxis_mode: String,
    dl_tx: Arc<watch::Sender<DownloadProgress>>,
    dl_active: Arc<AtomicBool>,
    model_path: PathBuf,
    engine: Arc<LlmEngine>,
    engine_dl_tx: Arc<watch::Sender<DownloadProgress>>,
    engine_dl_active: Arc<AtomicBool>,
    chatbot: Arc<Chatbot>,
}

const DASHBOARD: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width,initial-scale=1.0">
  <title>PYXIS-MCP</title>
  <style>
    *{box-sizing:border-box;margin:0;padding:0}
    body{font-family:monospace;background:#0f0f0f;color:#e0e0e0;padding:24px}
    h1{font-size:1.2rem;margin-bottom:24px;color:#fff}
    h2{font-size:.8rem;color:#888;text-transform:uppercase;letter-spacing:.1em;margin-bottom:12px}
    .status{display:flex;gap:12px;margin-bottom:32px;flex-wrap:wrap}
    .badge{padding:5px 12px;border-radius:4px;font-size:.8rem}
    .mock{background:#1a3a1a;color:#4caf50;border:1px solid #4caf50}
    .live{background:#1a2a3a;color:#2196f3;border:1px solid #2196f3}
    .ok{background:#1a3a1a;color:#4caf50;border:1px solid #4caf50}
    .warn{background:#3a2a00;color:#ff9800;border:1px solid #ff9800}
    section{margin-bottom:40px}
    table{width:100%;border-collapse:collapse;font-size:.85rem}
    th{text-align:left;padding:8px;border-bottom:1px solid #333;color:#888;font-weight:normal}
    td{padding:8px;border-bottom:1px solid #1a1a1a}
    .btn{padding:4px 12px;border:none;border-radius:3px;cursor:pointer;font-family:monospace;font-size:.8rem}
    .btn-approve{background:#1a3a1a;color:#4caf50;border:1px solid #4caf50}
    .btn-reject{background:#3a1a1a;color:#f44336;border:1px solid #f44336}
    .model-box{background:#1a1a1a;border:1px solid #333;border-radius:6px;padding:20px;max-width:600px}
    .model-row{display:flex;gap:8px;margin-bottom:12px}
    .model-row input{flex:1;background:#0f0f0f;border:1px solid #333;color:#e0e0e0;padding:8px;font-family:monospace;border-radius:3px;font-size:.85rem}
    .btn-dl{background:#1a2a3a;color:#2196f3;border:1px solid #2196f3;padding:8px 16px}
    progress{width:100%;height:5px;margin-top:8px;accent-color:#2196f3}
    .dl-status{font-size:.75rem;color:#666;margin-top:6px}
    .empty{color:#555;font-size:.85rem;padding:16px 0}
    .Ambient{color:#4caf50}.Cold{color:#2196f3}.Heavy{color:#ff9800}
    .Empty{color:#4caf50}.Occupied{color:#f44336}.Reserved{color:#ff9800}
    .Pending{color:#ff9800}.Approved{color:#4caf50}.Rejected{color:#f44336}
    .chat-box{background:#1a1a1a;border:1px solid #333;border-radius:6px;padding:20px;max-width:700px}
    .chat-history{min-height:120px;max-height:340px;overflow-y:auto;margin-bottom:12px;display:flex;flex-direction:column;gap:8px}
    .msg{padding:8px 12px;border-radius:4px;font-size:.85rem;line-height:1.5;max-width:90%}
    .msg.user{background:#1a2a3a;color:#90caf9;align-self:flex-end}
    .msg.assistant{background:#1e1e1e;border:1px solid #333;color:#e0e0e0;align-self:flex-start}
    .msg.tool-log{background:#0f1a0f;border:1px solid #2a4a2a;color:#81c784;font-size:.75rem;font-family:monospace;align-self:flex-start}
    .chat-input-row{display:flex;gap:8px}
    .chat-input-row input{flex:1;background:#0f0f0f;border:1px solid #333;color:#e0e0e0;padding:8px;font-family:monospace;border-radius:3px;font-size:.85rem}
    .btn-send{background:#1a2a3a;color:#2196f3;border:1px solid #2196f3;padding:8px 16px}
    .btn-send:disabled{opacity:.4;cursor:not-allowed}
    .thinking{color:#666;font-style:italic;font-size:.8rem}
  </style>
</head>
<body>
  <h1>PYXIS-MCP</h1>
  <div class="status" id="status-bar">Loading&hellip;</div>

  <section>
    <h2>Bins</h2>
    <table><thead><tr>
      <th>ID</th><th>Aisle</th><th>Row</th><th>Col</th><th>Zone</th><th>Cap&nbsp;(kg)</th><th>Status</th><th>SKU</th>
    </tr></thead>
    <tbody id="bins-body"><tr><td colspan="8" class="empty">Loading&hellip;</td></tr></tbody></table>
  </section>

  <section>
    <h2>Staged Moves</h2>
    <table><thead><tr>
      <th>ID</th><th>SKU</th><th>From</th><th>To</th><th>User</th><th>Status</th><th>Actions</th>
    </tr></thead>
    <tbody id="moves-body"><tr><td colspan="7" class="empty">Loading&hellip;</td></tr></tbody></table>
  </section>

  <section>
    <h2>Model Setup</h2>
    <div class="model-box">
      <p style="font-size:.85rem;color:#888;margin-bottom:12px">Paste a direct GGUF download URL. HuggingFace format: <code style="color:#aaa">huggingface.co/{org}/{repo}/resolve/main/{file}.gguf</code></p>
      <div class="model-row">
        <input type="text" id="model-url" value="https://huggingface.co/unsloth/gemma-4-E2B-it-GGUF/resolve/main/gemma-4-E2B-it-Q4_K_M.gguf">
        <button class="btn btn-dl" onclick="startDownload()">Download</button>
      </div>
      <progress id="dl-bar" value="0" max="100"></progress>
      <div class="dl-status" id="dl-status">Idle</div>
      <div id="model-actions" style="display:none;margin-top:12px">
        <button class="btn btn-reject" onclick="deleteModel()">Delete model</button>
        <span id="delete-status" style="font-size:.75rem;color:#666;margin-left:8px"></span>
      </div>
    </div>
  </section>

  <section>
    <h2>Engine Setup (llama-server)</h2>
    <div class="model-box">
      <p style="font-size:.85rem;color:#888;margin-bottom:12px">Download llama-server. Choose a build that matches your GPU. The binary and DLLs are extracted automatically from the ZIP.</p>
      <div style="display:flex;gap:6px;flex-wrap:wrap;margin-bottom:10px">
        <button class="btn btn-approve" style="font-size:.75rem" onclick="setEngineUrl('vulkan')">Vulkan (recommended)</button>
        <button class="btn" style="font-size:.75rem;background:#222;border:1px solid #555;color:#aaa" onclick="setEngineUrl('cpu')">CPU only</button>
        <button class="btn" style="font-size:.75rem;background:#222;border:1px solid #555;color:#aaa" onclick="setEngineUrl('cuda12')">CUDA 12 (NVIDIA)</button>
        <button class="btn" style="font-size:.75rem;background:#222;border:1px solid #555;color:#aaa" onclick="setEngineUrl('cuda13')">CUDA 13 (NVIDIA)</button>
      </div>
      <div class="model-row">
        <input type="text" id="engine-url" value="https://github.com/ggml-org/llama.cpp/releases/download/b8708/llama-b8708-bin-win-vulkan-x64.zip">
        <button class="btn btn-dl" onclick="startEngineDownload()">Download</button>
      </div>
      <progress id="engine-dl-bar" value="0" max="100"></progress>
      <div class="dl-status" id="engine-dl-status">Idle</div>
      <div style="margin-top:12px;display:flex;gap:8px;align-items:center;flex-wrap:wrap">
        <button id="engine-start-btn" class="btn btn-approve" style="display:none" onclick="startEngine()">Start Engine</button>
        <button class="btn" style="background:#222;border:1px solid #555;color:#aaa;font-size:.75rem" onclick="showEngineLog()">View log</button>
      </div>
      <pre id="engine-log" style="display:none;margin-top:10px;background:#0a0a0a;border:1px solid #222;padding:10px;font-size:.72rem;color:#888;max-height:180px;overflow-y:auto;white-space:pre-wrap"></pre>
    </div>
  </section>

  <section id="chat-section" style="display:none">
    <h2>PYXIS Chat</h2>
    <div class="chat-box">
      <div class="chat-history" id="chat-history"></div>
      <div class="chat-input-row">
        <input type="text" id="chat-input" placeholder="Ask about bins, inventory, or propose a move…" onkeydown="if(event.key==='Enter')sendChat()">
        <button class="btn btn-send" id="send-btn" onclick="sendChat()">Send</button>
      </div>
    </div>
  </section>

  <script>
    async function loadStatus(){
      try{
        const d=await(await fetch('/api/status')).json();
        document.getElementById('status-bar').innerHTML=`
          <span class="badge ${d.pyxis_mode==='Mock'?'mock':'live'}">${d.pyxis_mode} PYXIS</span>
          <span class="badge ok">${d.index_size} bins indexed</span>
          <span class="badge ${d.model_ready?'ok':'warn'}">${d.model_ready?'Model ready':'No model'}</span>
          <span class="badge ${d.engine_ready?'ok':'warn'}">${d.engine_ready?'Engine ready':'No engine'}</span>
          <span class="badge ${d.engine_running?'ok':'warn'}">${d.engine_running?'LLM running':'LLM stopped'}</span>`;
        document.getElementById('model-actions').style.display=d.model_ready?'block':'none';
        document.getElementById('engine-start-btn').style.display=d.engine_ready&&!d.engine_running?'inline-block':'none';
        document.getElementById('chat-section').style.display=d.engine_running?'block':'none';
      }catch(e){}
    }
    async function deleteModel(){
      if(!confirm('Delete the downloaded model file?'))return;
      const st=document.getElementById('delete-status');
      st.textContent='Deleting…';
      const r=await fetch('/api/model',{method:'DELETE'});
      st.textContent=r.ok?'Deleted.':'Error: '+r.status;
      loadStatus();
    }
    async function loadBins(){
      try{
        const bins=await(await fetch('/api/bins')).json();
        const tb=document.getElementById('bins-body');
        if(!bins.length){tb.innerHTML='<tr><td colspan="8" class="empty">No bins</td></tr>';return;}
        tb.innerHTML=bins.map(b=>`<tr>
          <td>${b.id}</td><td>${b.aisle}</td><td>${b.row}</td><td>${b.col}</td>
          <td class="${b.zone}">${b.zone}</td><td>${b.weight_cap_kg}</td>
          <td class="${b.status}">${b.status}</td><td>${b.current_sku||'—'}</td>
        </tr>`).join('');
      }catch(e){}
    }
    async function loadMoves(){
      try{
        const moves=await(await fetch('/api/staged')).json();
        const tb=document.getElementById('moves-body');
        if(!moves.length){tb.innerHTML='<tr><td colspan="7" class="empty">No staged moves</td></tr>';return;}
        tb.innerHTML=moves.map(m=>`<tr>
          <td>${m.id.slice(0,8)}</td><td>${m.sku_id}</td><td>${m.from_bin}</td><td>${m.to_bin}</td>
          <td>${m.user_id}</td><td class="${m.status}">${m.status}</td>
          <td>${m.status==='Pending'?`
            <button class="btn btn-approve" onclick="updateMove('${m.id}','Approved')">Approve</button>
            <button class="btn btn-reject" onclick="updateMove('${m.id}','Rejected')">Reject</button>
          `:'—'}</td>
        </tr>`).join('');
      }catch(e){}
    }
    async function updateMove(id,status){
      await fetch(`/api/staged/${id}`,{method:'PUT',headers:{'Content-Type':'application/json'},body:JSON.stringify({status})});
      loadMoves();
    }
    function startDownload(){
      const url=document.getElementById('model-url').value.trim();
      if(!url)return;
      document.getElementById('dl-status').textContent='Starting…';
      fetch('/api/model/download',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({url})});
      const es=new EventSource('/api/model/progress');
      es.onmessage=e=>{
        const d=JSON.parse(e.data);
        document.getElementById('dl-bar').value=d.pct;
        document.getElementById('dl-status').textContent=d.done?'Download complete!':d.error?`Error: ${d.error}`:`${d.pct}% — ${(d.bytes_done/1e6).toFixed(1)} / ${(d.total/1e6).toFixed(1)} MB`;
        if(d.done||d.error){es.close();loadStatus();}
      };
    }
    const ENGINE_URLS={
      vulkan:'https://github.com/ggml-org/llama.cpp/releases/download/b8708/llama-b8708-bin-win-vulkan-x64.zip',
      cpu:'https://github.com/ggml-org/llama.cpp/releases/download/b8708/llama-b8708-bin-win-cpu-x64.zip',
      cuda12:'https://github.com/ggml-org/llama.cpp/releases/download/b8708/llama-b8708-bin-win-cuda-12.4-x64.zip',
      cuda13:'https://github.com/ggml-org/llama.cpp/releases/download/b8708/llama-b8708-bin-win-cuda-13.1-x64.zip',
    };
    function setEngineUrl(k){document.getElementById('engine-url').value=ENGINE_URLS[k]||'';}
    async function showEngineLog(){
      const pre=document.getElementById('engine-log');
      pre.style.display=pre.style.display==='none'?'block':'none';
      if(pre.style.display==='block'){
        const r=await fetch('/api/engine/log');
        pre.textContent=r.ok?await r.text():'No log available.';
      }
    }
    function startEngineDownload(){
      const url=document.getElementById('engine-url').value.trim();
      if(!url)return;
      document.getElementById('engine-dl-status').textContent='Starting…';
      fetch('/api/engine/download',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({url})});
      const es=new EventSource('/api/engine/progress');
      es.onmessage=e=>{
        const d=JSON.parse(e.data);
        document.getElementById('engine-dl-bar').value=d.pct;
        document.getElementById('engine-dl-status').textContent=d.done?'Download complete!':d.error?`Error: ${d.error}`:`${d.pct}% — ${(d.bytes_done/1e6).toFixed(1)} / ${(d.total/1e6).toFixed(1)} MB`;
        if(d.done||d.error){es.close();loadStatus();}
      };
    }
    async function startEngine(){
      document.getElementById('engine-start-btn').textContent='Starting…';
      await fetch('/api/engine/start',{method:'POST'});
      setTimeout(loadStatus,2000);
    }

    let chatHistory=[];
    async function sendChat(){
      const input=document.getElementById('chat-input');
      const msg=input.value.trim();
      if(!msg)return;
      input.value='';
      const btn=document.getElementById('send-btn');
      btn.disabled=true;
      appendMsg('user',msg);
      const thinking=appendMsg('assistant','<span class="thinking">Thinking…</span>');
      try{
        const r=await fetch('/api/chat',{
          method:'POST',
          headers:{'Content-Type':'application/json'},
          body:JSON.stringify({message:msg,history:chatHistory})
        });
        if(!r.ok){const t=await r.text();thinking.innerHTML=`<em style="color:#f44336">Error: ${t}</em>`;btn.disabled=false;return;}
        const d=await r.json();
        chatHistory.push({role:'user',content:msg});
        chatHistory.push({role:'assistant',content:d.reply});
        thinking.innerHTML=d.reply.replace(/\n/g,'<br>');
        if(d.tool_calls&&d.tool_calls.length){
          d.tool_calls.forEach(([name,result])=>{
            appendMsg('tool-log',`▶ ${name}\n${JSON.stringify(JSON.parse(result),null,2)}`);
          });
        }
      }catch(e){thinking.innerHTML=`<em style="color:#f44336">Request failed</em>`;}
      btn.disabled=false;
    }
    function appendMsg(role,html){
      const el=document.createElement('div');
      el.className=`msg ${role}`;
      el.innerHTML=html;
      const hist=document.getElementById('chat-history');
      hist.appendChild(el);
      hist.scrollTop=hist.scrollHeight;
      return el;
    }

    function refresh(){loadStatus();loadBins();loadMoves();}
    refresh();setInterval(refresh,5000);
  </script>
</body>
</html>"#;

pub async fn serve(
    port: u16,
    pyxis: Arc<PyxisClient>,
    index: Arc<Mutex<VectorIndex>>,
    pyxis_mode: String,
    model_path: PathBuf,
    engine: Arc<LlmEngine>,
    chatbot: Arc<Chatbot>,
) {
    let (dl_tx, _) = watch::channel(DownloadProgress::default());
    let (engine_dl_tx, _) = watch::channel(DownloadProgress::default());
    let state = Arc::new(GuiState {
        pyxis,
        index,
        pyxis_mode,
        dl_tx: Arc::new(dl_tx),
        dl_active: Arc::new(AtomicBool::new(false)),
        model_path,
        engine,
        engine_dl_tx: Arc::new(engine_dl_tx),
        engine_dl_active: Arc::new(AtomicBool::new(false)),
        chatbot,
    });

    let app = Router::new()
        .route("/", get(dashboard))
        .route("/api/status", get(api_status))
        .route("/api/bins", get(api_bins))
        .route("/api/staged", get(api_staged))
        .route("/api/staged/{id}", put(api_update_move))
        .route("/api/model/download", post(api_model_download))
        .route("/api/model/progress", get(api_model_progress))
        .route("/api/model", delete(api_model_delete))
        .route("/api/engine/download", post(api_engine_download))
        .route("/api/engine/progress", get(api_engine_progress))
        .route("/api/engine/start", post(api_engine_start))
        .route("/api/engine/log", get(api_engine_log))
        .route("/api/chat", post(api_chat))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();
    eprintln!("GUI listening on http://localhost:{port}");
    axum::serve(listener, app).await.unwrap();
}

async fn dashboard() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], DASHBOARD)
}

#[derive(Serialize)]
struct StatusResponse {
    pyxis_mode: String,
    index_size: usize,
    model_ready: bool,
    engine_ready: bool,
    engine_running: bool,
}

async fn api_status(State(s): State<Arc<GuiState>>) -> Json<StatusResponse> {
    let index_size = s.index.lock().unwrap().len();
    let model_ready = s.model_path.exists();
    let engine_ready = s.engine.is_ready();
    let engine_running = s.engine.is_running();
    Json(StatusResponse { pyxis_mode: s.pyxis_mode.clone(), index_size, model_ready, engine_ready, engine_running })
}

async fn api_bins(State(s): State<Arc<GuiState>>) -> impl IntoResponse {
    match s.pyxis.list_bins().await {
        Ok(bins) => Json(bins).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}

async fn api_staged(State(s): State<Arc<GuiState>>) -> impl IntoResponse {
    match s.pyxis.list_staged().await {
        Ok(moves) => Json(moves).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct UpdateMoveBody {
    status: MoveStatus,
}

async fn api_update_move(
    State(s): State<Arc<GuiState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateMoveBody>,
) -> impl IntoResponse {
    match s.pyxis.update_move(&id, body.status).await {
        Ok(mv) => Json(mv).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct DownloadRequest {
    url: String,
}

async fn api_model_download(
    State(s): State<Arc<GuiState>>,
    Json(req): Json<DownloadRequest>,
) -> impl IntoResponse {
    if s.dl_active.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return (StatusCode::CONFLICT, "Download already in progress").into_response();
    }
    let tx = s.dl_tx.clone();
    let active = s.dl_active.clone();
    let model_path = s.model_path.clone();
    let url = req.url;
    tokio::spawn(async move {
        run_download(url, tx, model_path).await;
        active.store(false, Ordering::SeqCst);
    });
    StatusCode::ACCEPTED.into_response()
}

async fn run_download(url: String, tx: Arc<watch::Sender<DownloadProgress>>, path: PathBuf) {
    use tokio::io::AsyncWriteExt;

    let resp = reqwest::get(&url).await;
    let mut resp = match resp {
        Err(e) => {
            tx.send(DownloadProgress { error: Some(e.to_string()), done: true, ..Default::default() }).ok();
            return;
        }
        Ok(r) => r,
    };

    let content_type = resp.headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if content_type.contains("text/html") {
        tx.send(DownloadProgress {
            error: Some("URL returned an HTML page, not a binary file. For HuggingFace use the /resolve/main/ path: https://huggingface.co/{org}/{repo}/resolve/main/{file}.gguf".into()),
            done: true, ..Default::default()
        }).ok();
        return;
    }

    let total = resp.content_length().unwrap_or(0);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    let mut file = match tokio::fs::File::create(&path).await {
        Err(e) => {
            tx.send(DownloadProgress { error: Some(e.to_string()), done: true, ..Default::default() }).ok();
            return;
        }
        Ok(f) => f,
    };

    let mut bytes_done = 0u64;
    loop {
        match resp.chunk().await {
            Err(e) => {
                tx.send(DownloadProgress { error: Some(e.to_string()), done: true, ..Default::default() }).ok();
                return;
            }
            Ok(None) => break,
            Ok(Some(chunk)) => {
                if file.write_all(&chunk).await.is_err() { break; }
                bytes_done += chunk.len() as u64;
                let pct = if total > 0 { (bytes_done * 100 / total) as u8 } else { 0 };
                tx.send(DownloadProgress { pct, bytes_done, total, done: false, error: None }).ok();
            }
        }
    }
    tx.send(DownloadProgress { pct: 100, bytes_done, total, done: true, error: None }).ok();
}

async fn run_engine_download_zip(url: String, tx: Arc<watch::Sender<DownloadProgress>>, path: PathBuf) {
    use std::io::Read;
    use tokio::io::AsyncWriteExt;

    let resp = match reqwest::get(&url).await {
        Err(e) => {
            tx.send(DownloadProgress { error: Some(e.to_string()), done: true, ..Default::default() }).ok();
            return;
        }
        Ok(r) => r,
    };

    let total = resp.content_length().unwrap_or(0);
    let mut bytes_done = 0u64;
    let mut body: Vec<u8> = Vec::with_capacity(total as usize);
    let mut resp = resp;

    loop {
        match resp.chunk().await {
            Err(e) => {
                tx.send(DownloadProgress { error: Some(e.to_string()), done: true, ..Default::default() }).ok();
                return;
            }
            Ok(None) => break,
            Ok(Some(chunk)) => {
                body.extend_from_slice(&chunk);
                bytes_done += chunk.len() as u64;
                let pct = if total > 0 { (bytes_done * 100 / total) as u8 } else { 0 };
                tx.send(DownloadProgress { pct, bytes_done, total, done: false, error: None }).ok();
            }
        }
    }

    let cursor = std::io::Cursor::new(body);
    let mut archive = match zip::ZipArchive::new(cursor) {
        Ok(a) => a,
        Err(e) => {
            tx.send(DownloadProgress { error: Some(format!("ZIP error: {e}")), done: true, ..Default::default() }).ok();
            return;
        }
    };

    let engine_dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("engines"));
    let target_exe = if cfg!(windows) { "llama-server.exe" } else { "llama-server" };

    // collect all files to extract (exe + dlls) — sync, before any await
    let mut to_write: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    let mut found_exe = false;
    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) { Ok(e) => e, Err(_) => continue };
        if entry.is_dir() { continue; }
        let raw_name = entry.name().to_string();
        let file_name = raw_name.split(['/', '\\']).next_back().unwrap_or("").to_string();
        let is_exe = file_name == target_exe;
        let is_dll = file_name.to_lowercase().ends_with(".dll");
        if !is_exe && !is_dll { continue; }
        let mut content = Vec::new();
        if entry.read_to_end(&mut content).is_err() { continue; }
        if is_exe { found_exe = true; }
        to_write.push((engine_dir.join(&file_name), content));
    }

    if !found_exe {
        tx.send(DownloadProgress {
            error: Some(format!("'{target_exe}' not found in ZIP")),
            done: true, ..Default::default()
        }).ok();
        return;
    }

    tokio::fs::create_dir_all(&engine_dir).await.ok();
    for (dest, content) in to_write {
        match tokio::fs::File::create(&dest).await {
            Err(e) => {
                tx.send(DownloadProgress { error: Some(e.to_string()), done: true, ..Default::default() }).ok();
                return;
            }
            Ok(mut f) => { f.write_all(&content).await.ok(); }
        }
    }

    tx.send(DownloadProgress { pct: 100, bytes_done, total, done: true, error: None }).ok();
}

async fn api_model_progress(
    State(s): State<Arc<GuiState>>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = s.dl_tx.subscribe();
    let stream = WatchStream::new(rx).map(|p| {
        let data = serde_json::to_string(&p).unwrap_or_default();
        Ok(Event::default().data(data))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn api_model_delete(State(s): State<Arc<GuiState>>) -> impl IntoResponse {
    if s.dl_active.load(Ordering::SeqCst) {
        return (StatusCode::CONFLICT, "Download in progress").into_response();
    }
    match tokio::fs::remove_file(&s.model_path).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn api_engine_download(
    State(s): State<Arc<GuiState>>,
    Json(req): Json<DownloadRequest>,
) -> impl IntoResponse {
    if s.engine_dl_active.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return (StatusCode::CONFLICT, "Download already in progress").into_response();
    }
    let tx = s.engine_dl_tx.clone();
    let active = s.engine_dl_active.clone();
    let engine_path = s.engine.engine_path.clone();
    let url = req.url;
    tokio::spawn(async move {
        if url.to_lowercase().ends_with(".zip") {
            run_engine_download_zip(url, tx, engine_path.clone()).await;
        } else {
            run_download(url, tx, engine_path.clone()).await;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if engine_path.exists() {
                let _ = std::fs::set_permissions(&engine_path, std::fs::Permissions::from_mode(0o755));
            }
        }
        active.store(false, Ordering::SeqCst);
    });
    StatusCode::ACCEPTED.into_response()
}

async fn api_engine_progress(
    State(s): State<Arc<GuiState>>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = s.engine_dl_tx.subscribe();
    let stream = WatchStream::new(rx).map(|p| {
        let data = serde_json::to_string(&p).unwrap_or_default();
        Ok(Event::default().data(data))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn api_engine_log(State(s): State<Arc<GuiState>>) -> impl IntoResponse {
    let log_path = s.engine.engine_dir.join("llama-server.log");
    match tokio::fs::read_to_string(&log_path).await {
        Ok(content) => (StatusCode::OK, content).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "No log yet").into_response(),
    }
}

async fn api_engine_start(State(s): State<Arc<GuiState>>) -> impl IntoResponse {
    if s.engine.is_running() {
        return (StatusCode::OK, "already running").into_response();
    }
    let engine = s.engine.clone();
    tokio::spawn(async move {
        if let Err(e) = engine.start().await {
            eprintln!("engine start error: {e}");
        }
    });
    StatusCode::ACCEPTED.into_response()
}

#[derive(Deserialize)]
struct ChatRequest {
    message: String,
    history: Option<Vec<Message>>,
}

async fn api_chat(
    State(s): State<Arc<GuiState>>,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    if !s.engine.is_running() {
        return (StatusCode::SERVICE_UNAVAILABLE, "LLM engine not running. Start it from the Engine Setup section.").into_response();
    }
    let history = req.history.unwrap_or_default();
    match s.chatbot.chat(history, &req.message).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
