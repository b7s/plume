use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use futures_util::StreamExt;

const GITHUB_REPO: &str = "ggml-org/llama.cpp";
const USER_AGENT: &str = "Plume/0.1";

pub static LLAMA_PROCESS: Mutex<Option<Child>> = Mutex::new(None);

/// Kill the managed llama-server child (if running) plus any orphaned
/// `llama-server.exe` processes left behind by a hard exit (common in dev).
/// Without this, a stale server can keep holding our port and serving the
/// previous model, which makes a model change look like it "wasn't recognized".
pub fn kill_all() {
    if let Ok(mut proc) = LLAMA_PROCESS.lock() {
        if let Some(ref mut child) = *proc {
            let _ = child.kill();
            let _ = child.wait();
        }
        *proc = None;
    }
    // Mop up orphans so they can't keep our port alive.
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "llama-server.exe"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();
}

/// Auto-construct a HuggingFace download URL from a GGUF model filename.
/// e.g. "Qwen3-0.6B-Q4_K_M.gguf" -> "unsloth/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q4_K_M.gguf"
fn resolve_model_url(model: &str) -> String {
    let name = model.trim_end_matches(".gguf");
    let parts: Vec<&str> = name.split('-').collect();

    let mut base_parts: Vec<&str> = Vec::new();
    for &part in &parts {
        let is_quant = is_quant_suffix(part);
        if is_quant {
            break;
        }
        base_parts.push(part);
    }

    if base_parts.is_empty() {
        return format!("https://huggingface.co/unsloth/{name}-GGUF/resolve/main/{model}");
    }

    let base = base_parts.join("-");
    format!("https://huggingface.co/unsloth/{base}-GGUF/resolve/main/{model}")
}

/// Check if a filename part is a quantization suffix (Q4_K_M, IQ2_M, BF16, UD-*, etc.)
fn is_quant_suffix(part: &str) -> bool {
    if part.starts_with('Q') && part.len() >= 2 {
        if let Some(c) = part.chars().nth(1) {
            if c.is_ascii_digit() {
                return true;
            }
        }
    }
    if part.starts_with("IQ") && part.len() >= 3 {
        if let Some(c) = part.chars().nth(2) {
            if c.is_ascii_digit() {
                return true;
            }
        }
    }
    part == "BF16" || part == "F16" || part.starts_with("UD")
}

pub struct LlamaSetup {
    llama_dir: PathBuf,
    models_dir: PathBuf,
}

impl LlamaSetup {
    pub fn new() -> Self {
        let appdata = std::env::var("APPDATA")
            .or_else(|_| std::env::var("LOCALAPPDATA"))
            .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Roaming".into());
        Self {
            llama_dir: PathBuf::from(&appdata).join("plume").join("llama"),
            models_dir: PathBuf::from(&appdata).join("plume").join("models"),
        }
    }

    pub fn find_server(&self) -> Option<PathBuf> {
        if let Ok(output) = Command::new("where").arg("llama-server.exe").output() {
            if output.status.success() {
                if let Ok(stdout) = String::from_utf8(output.stdout) {
                    if let Some(line) = stdout.lines().next() {
                        let p = PathBuf::from(line.trim());
                        if p.exists() {
                            return Some(p);
                        }
                    }
                }
            }
        }
        let ours = self.llama_dir.join("llama-server.exe");
        if ours.exists() {
            return Some(ours);
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let bundled = parent.join("llama-server.exe");
                if bundled.exists() {
                    return Some(bundled);
                }
            }
        }
        None
    }

    pub async fn install(&self) -> Result<(), String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("client: {e}"))?;

        let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
        let resp = client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| format!("release fetch: {e}"))?;

        let data: serde_json::Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;

        let zip_url = data["assets"]
            .as_array()
            .ok_or("no assets array")?
            .iter()
            .find(|a| {
                a["name"]
                    .as_str()
                    .is_some_and(|n| n.contains("win-cpu-x64") && n.ends_with(".zip"))
            })
            .and_then(|a| a["browser_download_url"].as_str())
            .ok_or("no win-cpu-x64 zip asset")?;

        eprintln!("[plume] Downloading llama.cpp from {zip_url}");
        let zip_bytes = client
            .get(zip_url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| format!("download: {e}"))?
            .bytes()
            .await
            .map_err(|e| format!("read: {e}"))?;

        std::fs::create_dir_all(&self.llama_dir).map_err(|e| format!("mkdir: {e}"))?;

        let cursor = Cursor::new(zip_bytes.to_vec());
        let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("zip: {e}"))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| format!("zip entry {i}: {e}"))?;
            let fname = std::path::Path::new(file.name())
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file.name())
                .to_owned();
            let outpath = self.llama_dir.join(&fname);

            if file.is_dir() {
                let _ = std::fs::create_dir_all(&outpath);
            } else {
                if let Some(parent) = outpath.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let mut outfile =
                    std::fs::File::create(&outpath).map_err(|e| format!("create {fname}: {e}"))?;
                std::io::copy(&mut file, &mut outfile)
                    .map_err(|e| format!("extract {fname}: {e}"))?;
            }
        }

        eprintln!("[plume] llama-server installed to {:?}", self.llama_dir);
        Ok(())
    }

    pub fn model_path(&self, name: &str) -> PathBuf {
        self.models_dir.join(name)
    }

    pub fn model_exists(&self, name: &str) -> bool {
        self.model_path(name).exists()
    }

    pub async fn download_model(&self, name: &str, url: &str) -> Result<PathBuf, String> {
        let path = self.model_path(name);
        if path.exists() {
            return Ok(path);
        }

        std::fs::create_dir_all(&self.models_dir).map_err(|e| format!("mkdir: {e}"))?;

        let final_url = if url.is_empty() {
            resolve_model_url(name)
        } else {
            url.to_string()
        };

        eprintln!("[plume] Downloading model {name} from {final_url}");
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .map_err(|e| format!("client: {e}"))?;

        let response = client
            .get(&final_url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| format!("get: {e}"))?;

        let total = response.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let mut file = std::fs::File::create(&path).map_err(|e| format!("create: {e}"))?;
        let mut stream = response.bytes_stream();
        let mut last_print = 0u64;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("stream: {e}"))?;
            file.write_all(&chunk).map_err(|e| format!("write: {e}"))?;
            downloaded += chunk.len() as u64;
            if total > 0 && downloaded - last_print > 50 * 1024 * 1024 {
                let pct = (downloaded as f64 / total as f64) * 100.0;
                eprintln!(
                    "[plume] Model: {pct:.0}% ({}/{} MB)",
                    downloaded / (1024 * 1024),
                    total / (1024 * 1024)
                );
                last_print = downloaded;
            }
        }

        eprintln!("[plume] Model saved: {:?}", path);
        Ok(path)
    }

    pub fn start_server(port: u16, model_path: &PathBuf, server_path: &PathBuf) -> Result<Child, String> {
        let mut cmd = Command::new(server_path);
        cmd.arg("-m")
            .arg(model_path)
            .arg("--port")
            .arg(port.to_string())
            .arg("--host")
            .arg("127.0.0.1")
            .arg("-c")
            .arg("2048")
            .arg("--no-warmup")
            .stdin(Stdio::null());
        // Show logs in dev (npm run tauri dev), hide in production builds.
        if cfg!(debug_assertions) {
            cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        } else {
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
        }
        let child = cmd.spawn().map_err(|e| format!("spawn: {e}"))?;
        Ok(child)
    }

    pub async fn wait_ready(port: u16, timeout: Duration) -> Result<(), String> {
        let url = format!("http://127.0.0.1:{port}/health");
        let start = std::time::Instant::now();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| format!("client: {e}"))?;

        loop {
            if start.elapsed() > timeout {
                return Err("server not ready in time".into());
            }
            match client.get(&url).send().await {
                Ok(r) if r.status().is_success() => return Ok(()),
                _ => tokio::time::sleep(Duration::from_millis(500)).await,
            }
        }
    }

    pub async fn ensure_ready(
        model_name: &str,
        model_url: &str,
        _port: u16,
        install_server: bool,
        download_model: bool,
    ) -> Result<(PathBuf, PathBuf), String> {
        let setup = Self::new();

        let server_path = if let Some(p) = setup.find_server() {
            p
        } else if install_server {
            eprintln!("[plume] llama-server not found — installing...");
            setup.install().await?;
            setup.find_server().ok_or("install succeeded but server not found")?
        } else {
            return Err("llama-server not installed and auto-install disabled".into());
        };

        let model_path = if setup.model_exists(model_name) {
            setup.model_path(model_name)
        } else if download_model {
            eprintln!("[plume] Model {model_name} not found — downloading...");
            setup.download_model(model_name, model_url).await?;
            setup.model_path(model_name)
        } else {
            return Err("model not found and auto-download disabled".into());
        };

        Ok((server_path, model_path))
    }
}
