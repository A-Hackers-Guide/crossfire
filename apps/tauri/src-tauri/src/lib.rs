//! Tauri shell for Crossfire. The engine is linked in-process, so the whole app
//! ships as one executable / bundle per OS.

use crossfire_engine::{
    BuildRequest, BuildResult, ConvertRequest, ConvertResult, Os, ProgressEvent, Question, ScanReport, Toolchain,
};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

const PROGRESS: &str = "crossfire://progress";

/// Run a blocking engine call off the UI thread, forwarding progress events to the webview.
async fn run<T, F>(app: AppHandle, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&dyn Fn(ProgressEvent)) -> anyhow::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let emit = |e: ProgressEvent| {
            let _ = app.emit(PROGRESS, e);
        };
        f(&emit).map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn host_os() -> Os {
    if cfg!(target_os = "windows") {
        Os::Windows
    } else if cfg!(target_os = "macos") {
        Os::Macos
    } else if cfg!(target_os = "freebsd") {
        Os::Freebsd
    } else {
        Os::Linux
    }
}

#[tauri::command]
async fn scan(app: AppHandle, path: String) -> Result<ScanReport, String> {
    run(app, move |on| crossfire_engine::scan(&PathBuf::from(path), on)).await
}

#[tauri::command]
fn questions(scan: ScanReport, targets: Vec<Os>) -> Vec<Question> {
    crossfire_engine::questions(&scan, &targets)
}

#[tauri::command]
async fn convert(app: AppHandle, req: ConvertRequest) -> Result<ConvertResult, String> {
    run(app, move |on| crossfire_engine::convert(&req, on)).await
}

#[tauri::command]
async fn toolchains() -> Vec<Toolchain> {
    tauri::async_runtime::spawn_blocking(crossfire_engine::toolchains).await.unwrap_or_default()
}

#[tauri::command]
async fn build(app: AppHandle, req: BuildRequest) -> Result<BuildResult, String> {
    run(app, move |on| crossfire_engine::build(&req, on)).await
}

#[tauri::command]
async fn test_with_gptk(app: AppHandle, exe: String) -> Result<String, String> {
    run(app, move |on| crossfire_engine::test_with_gptk(&PathBuf::from(exe), on)).await
}

/// Reveal a file or folder in the system file manager.
#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    let (cmd, args): (&str, Vec<&str>) = if cfg!(target_os = "windows") {
        ("explorer", vec![&path])
    } else if cfg!(target_os = "macos") {
        ("open", vec![&path])
    } else {
        ("xdg-open", vec![&path])
    };
    std::process::Command::new(cmd).args(args).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run_app() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            host_os, scan, questions, convert, toolchains, build, test_with_gptk, open_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running Crossfire");
}
