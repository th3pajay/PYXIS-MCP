use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

mod pyxis_client;
mod pyxis_mock;
mod chatbot;
mod gui;
mod llm_engine;
mod mcp_server;
mod models;
mod safety_validation;
mod tools;
mod vector_search;

use pyxis_client::PyxisClient;
use vector_search::VectorIndex;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (pyxis_url, pyxis_mode) = match std::env::var("PYXIS_BASE_URL") {
        Ok(url) => (url, "Live".to_string()),
        Err(_) => {
            tokio::spawn(async { pyxis_mock::serve(3030).await });
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            ("http://localhost:3030".to_string(), "Mock".to_string())
        }
    };

    let pyxis = Arc::new(PyxisClient::new(&pyxis_url));

    let bins = pyxis.list_bins().await?;
    let skus = pyxis.list_skus().await?;
    let mut index = VectorIndex::new()?;
    index.index_bins(&bins)?;
    index.index_skus(&skus)?;
    let index = Arc::new(Mutex::new(index));

    let model_path = PathBuf::from(
        std::env::var("MODEL_PATH").unwrap_or_else(|_| "models/gemma.gguf".to_string()),
    );
    let engine_path = PathBuf::from(
        std::env::var("ENGINE_PATH").unwrap_or_else(|_| {
            if cfg!(windows) { "engines/llama-server.exe".to_string() }
            else { "engines/llama-server".to_string() }
        }),
    );
    let ollama_model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gemma".to_string());

    let engine = Arc::new(llm_engine::LlmEngine::new(engine_path.clone(), model_path.clone(), 8181));

    if engine.is_ready() {
        let e = engine.clone();
        tokio::spawn(async move {
            if let Err(err) = e.start().await {
                eprintln!("LLM engine failed to start: {err}");
            }
        });
    }

    let chatbot = Arc::new(chatbot::Chatbot::new(
        pyxis.clone(),
        index.clone(),
        engine.base_url(),
        ollama_model,
    ));

    let gui_pyxis = pyxis.clone();
    let gui_index = index.clone();
    let gui_mode = pyxis_mode.clone();
    let gui_model_path = model_path.clone();

    tokio::select! {
        res = mcp_server::serve(pyxis, index) => {
            if let Err(e) = res { eprintln!("MCP server error: {e}"); }
        }
        _ = gui::serve(8080, gui_pyxis, gui_index, gui_mode, gui_model_path, engine, chatbot) => {}
    }

    Ok(())
}
