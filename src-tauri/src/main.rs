// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::{io::BufRead, io::BufReader, io::Read};
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;
use url::Url;

#[derive(Clone, serde::Serialize)]
struct DownloadProgressPayload {
    percentage: u8,
    speed: Option<String>,
    eta: Option<String>,
    status: String,
    done: bool,
    success: bool,
}

#[derive(Clone, serde::Serialize)]
struct DeepLinkPayload {
    url: String,
}

#[derive(Debug, Deserialize)]
struct EngineProgress {
    #[serde(default)]
    percentage: Option<Value>,
    #[serde(default)]
    progress: Option<Value>,
    #[serde(default)]
    speed: Option<String>,
    #[serde(default)]
    eta: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    done: Option<bool>,
    #[serde(default)]
    success: Option<bool>,
}

fn clamp_percentage(progress: &EngineProgress) -> u8 {
    fn parse_value(value: &Value) -> Option<f64> {
        match value {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => {
                let cleaned = s.replace('%', "").trim().to_string();
                cleaned.parse::<f64>().ok()
            }
            _ => None,
        }
    }

    let value = progress
        .percentage
        .as_ref()
        .and_then(parse_value)
        .or_else(|| progress.progress.as_ref().and_then(parse_value))
        .unwrap_or(0.0);
    value.clamp(0.0, 100.0).round() as u8
}

fn extract_download_url(arg: &str) -> Option<String> {
    let parsed = Url::parse(arg).ok()?;
    if parsed.scheme() != "vibefetch" || parsed.host_str() != Some("download") {
        return None;
    }

    parsed
        .query_pairs()
        .find_map(|(key, value)| (key == "url").then(|| value.to_string()))
}

fn emit_deeplink_url(app: &tauri::AppHandle, url: &str) {
    let _ = app.emit(
        "deep-link-received",
        DeepLinkPayload {
            url: url.to_string(),
        },
    );
}

fn run_engine_download(
    app: tauri::AppHandle,
    url: String,
    start_time: Option<String>,
    end_time: Option<String>,
) -> Result<String, String> {
    if url.trim().is_empty() {
        return Err("URL cannot be empty.".to_string());
    }

    let cwd = std::env::current_dir().map_err(|e| format!("Failed to read current dir: {e}"))?;
    let engine_path_candidates = [
        cwd.join("engine.py"),
        cwd.join("..").join("engine.py"),
        PathBuf::from("engine.py"),
    ];

    let engine_path = engine_path_candidates
        .into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| "Could not find engine.py.".to_string())?;

    let mut child = Command::new("py")
        .arg(engine_path)
        .arg(url)
        .arg(start_time.unwrap_or_default())
        .arg(end_time.unwrap_or_default())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to execute py: {e}"))?;

    let _ = app.emit(
        "download-progress",
        DownloadProgressPayload {
            percentage: 0,
            speed: None,
            eta: None,
            status: "Starting download...".to_string(),
            done: false,
            success: false,
        },
    );

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture engine stdout.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture engine stderr.".to_string())?;

    let stderr_handle = std::thread::spawn(move || {
        let mut err_reader = BufReader::new(stderr);
        let mut err_output = String::new();
        let _ = err_reader.read_to_string(&mut err_output);
        err_output
    });

    let mut last_percentage = 0_u8;
    let mut last_speed: Option<String> = None;
    let mut last_eta: Option<String> = None;

    let reader = BufReader::new(stdout);
    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };
        println!("[engine stdout] {}", line);

        if let Some(raw_json) = line.strip_prefix("PROGRESS:") {
            if let Ok(progress) = serde_json::from_str::<EngineProgress>(raw_json.trim()) {
                let pct = clamp_percentage(&progress);
                last_percentage = pct;
                if let Some(speed) = progress.speed.clone() {
                    last_speed = Some(speed);
                }
                if let Some(eta) = progress.eta.clone() {
                    last_eta = Some(eta);
                }

                let _ = app.emit(
                    "download-progress",
                    DownloadProgressPayload {
                        percentage: pct,
                        speed: progress.speed.or_else(|| last_speed.clone()),
                        eta: progress.eta.or_else(|| last_eta.clone()),
                        status: progress
                            .status
                            .unwrap_or_else(|| format!("Downloading... {}%", pct)),
                        done: progress.done.unwrap_or(false),
                        success: progress.success.unwrap_or(false),
                    },
                );
            }
        }
    }

    let process_status = child
        .wait()
        .map_err(|e| format!("Failed waiting for engine.py: {e}"))?;
    let stderr_output = stderr_handle.join().unwrap_or_default();
    if !stderr_output.trim().is_empty() {
        eprintln!("[engine stderr] {}", stderr_output.trim());
    }

    if process_status.success() {
        let _ = app.emit(
            "download-progress",
            DownloadProgressPayload {
                percentage: 100,
                speed: last_speed.clone(),
                eta: Some("0s".to_string()),
                status: "Download completed.".to_string(),
                done: true,
                success: true,
            },
        );
        Ok("Download completed.".to_string())
    } else {
        let msg = if stderr_output.trim().is_empty() {
            "engine.py failed with no error output.".to_string()
        } else {
            stderr_output.trim().to_string()
        };
        let _ = app.emit(
            "download-progress",
            DownloadProgressPayload {
                percentage: last_percentage,
                speed: last_speed,
                eta: last_eta,
                status: "Download failed.".to_string(),
                done: true,
                success: false,
            },
        );
        Err(msg)
    }
}

#[tauri::command]
fn download_with_engine(
    app: tauri::AppHandle,
    url: String,
    start_time: Option<String>,
    end_time: Option<String>,
) -> Result<String, String> {
    run_engine_download(app, url, start_time, end_time)
}

#[tauri::command]
fn download_video(
    app: tauri::AppHandle,
    url: String,
    start_time: Option<String>,
    end_time: Option<String>,
) -> Result<String, String> {
    run_engine_download(app, url, start_time, end_time)
}

#[tauri::command]
fn fetch_video_info(url: String) -> Result<String, String> {
    if url.trim().is_empty() {
        return Err("URL cannot be empty.".to_string());
    }
    Ok(format!("Fetch info triggered for {}", url))
}

#[tauri::command]
fn open_downloads() -> Result<(), String> {
    let user_profile =
        std::env::var("USERPROFILE").map_err(|e| format!("Missing USERPROFILE: {e}"))?;
    let downloads_path = PathBuf::from(user_profile).join("Downloads");

    Command::new("explorer")
        .arg(downloads_path)
        .spawn()
        .map_err(|e| format!("Failed to open Downloads folder: {e}"))?;

    Ok(())
}

fn main() {
    let initial_deep_link = std::env::args().find_map(|arg| extract_download_url(&arg));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }

            if let Some(link_url) = argv.iter().find_map(|arg| extract_download_url(arg)) {
                emit_deeplink_url(app, &link_url);
            }
        }))
        .setup(move |app| {
            #[cfg(desktop)]
            {
                app.deep_link().register("vibefetch")?;
                let app_handle = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    if let Some(link_url) = event.urls().iter().find_map(|url| extract_download_url(url.as_str())) {
                        emit_deeplink_url(&app_handle, &link_url);
                    }
                });
            }

            if let Some(link_url) = initial_deep_link.clone() {
                emit_deeplink_url(&app.handle(), &link_url);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            fetch_video_info,
            download_video,
            download_with_engine,
            open_downloads
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
