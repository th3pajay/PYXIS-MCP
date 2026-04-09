use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{bail, Result};

pub struct LlmEngine {
    process: Arc<Mutex<Option<Child>>>,
    port: u16,
    pub engine_path: PathBuf,
    pub engine_dir: PathBuf,
    pub model_path: PathBuf,
}

impl LlmEngine {
    pub fn new(engine_path: PathBuf, model_path: PathBuf, port: u16) -> Self {
        let engine_dir = engine_path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            process: Arc::new(Mutex::new(None)),
            port,
            engine_path,
            engine_dir,
            model_path,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.engine_path.exists() && self.model_path.exists()
    }

    pub fn is_running(&self) -> bool {
        let mut guard = self.process.lock().unwrap();
        if let Some(child) = guard.as_mut() {
            matches!(child.try_wait(), Ok(None))
        } else {
            false
        }
    }

    pub async fn start(&self) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }
        if !self.engine_path.exists() {
            bail!("engine binary not found at {:?}", self.engine_path);
        }
        if !self.model_path.exists() {
            bail!("model not found at {:?}", self.model_path);
        }

        let log_path = self.engine_dir.join("llama-server.log");
        let log_file = std::fs::File::create(&log_path)
            .unwrap_or_else(|_| std::fs::File::create(std::env::temp_dir().join("llama-server.log")).unwrap());
        let log_file2 = log_file.try_clone().unwrap();

        let abs_model = self.model_path.canonicalize()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(&self.model_path));

        let child = Command::new(&self.engine_path)
            .args([
                "--model", abs_model.to_str().unwrap_or(""),
                "--port", &self.port.to_string(),
                "--host", "127.0.0.1",
                "--ctx-size", "4096",
                "-ngl", "0",
            ])
            .current_dir(&self.engine_dir)
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_file2))
            .spawn()?;

        *self.process.lock().unwrap() = Some(child);
        self.wait_ready(120).await
    }

    async fn wait_ready(&self, timeout_secs: u64) -> Result<()> {
        let health_url = format!("http://127.0.0.1:{}/health", self.port);
        let client = reqwest::Client::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            {
                let mut guard = self.process.lock().unwrap();
                if let Some(child) = guard.as_mut() {
                    match child.try_wait() {
                        Ok(Some(status)) => bail!("engine exited early ({}). Check engines/llama-server.log", status),
                        Ok(None) => {}
                        Err(e) => bail!("engine process error: {e}"),
                    }
                }
            }
            if tokio::time::Instant::now() >= deadline {
                bail!("engine did not become ready within {}s. Check engines/llama-server.log", timeout_secs);
            }
            if client.get(&health_url).send().await.map(|r| r.status().is_success()).unwrap_or(false) {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn stop(&self) {
        if let Some(mut child) = self.process.lock().unwrap().take() {
            child.kill().ok();
        }
    }
}

impl Drop for LlmEngine {
    fn drop(&mut self) {
        self.stop();
    }
}
