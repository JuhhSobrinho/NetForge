mod project_io;

use netforge_engine::cli::{self, CliOutcome, CliSession};
use netforge_engine::sim::{self, DeviceTables, PingOutcome};
use netforge_engine::topology::Topology;
use serde::Serialize;
use std::path::PathBuf;

/// Status do motor, enviado ao frontend pelo comando `engine_status`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub engine_version: String,
}

/// Retorna o status do motor. Usado para validar a ida-e-volta frontend → Rust → frontend.
#[tauri::command]
fn engine_status() -> EngineStatus {
    EngineStatus {
        engine_version: netforge_engine::engine_version().to_string(),
    }
}

/// Salva o JSON do projeto no caminho escolhido pelo usuário no diálogo do sistema.
#[tauri::command]
fn save_project_file(path: String, contents: String) -> Result<(), String> {
    let path: PathBuf = project_io::validate_project_path(&path)?;
    project_io::write_project(&path, &contents)
}

/// Lê o JSON do projeto de um caminho escolhido pelo usuário.
#[tauri::command]
fn open_project_file(path: String) -> Result<String, String> {
    let path: PathBuf = project_io::validate_project_path(&path)?;
    project_io::read_project(&path)
}

/// Executa um ping no motor. O frontend envia a topologia atual; o motor valida tudo.
#[tauri::command]
fn run_ping(
    topology: Topology,
    source_device_id: String,
    target_ip: String,
    count: u8,
) -> Result<PingOutcome, String> {
    sim::run_ping(&topology, &source_device_id, &target_ip, count)
}

/// Salva a captura PCAP de um ping no caminho escolhido pelo usuário.
#[tauri::command]
fn export_pcap(path: String, bytes: Vec<u8>) -> Result<(), String> {
    let path: PathBuf = project_io::validate_path_with(&path, "pcap")?;
    project_io::write_capture(&path, &bytes)
}

/// Executa uma linha da CLI de um equipamento. O frontend guarda o modo entre as linhas.
#[tauri::command]
fn cli_execute(
    topology: Topology,
    device_id: String,
    session: CliSession,
    line: String,
    runtime: Option<Vec<DeviceTables>>,
) -> Result<CliOutcome, String> {
    cli::execute(&topology, &device_id, session, &line, runtime.as_deref())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .invoke_handler(tauri::generate_handler![
        engine_status,
        save_project_file,
        open_project_file,
        run_ping,
        export_pcap,
        cli_execute
    ])
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_status_reports_engine_version() {
        let status = engine_status();
        assert_eq!(status.engine_version, netforge_engine::engine_version());
    }

    #[test]
    fn engine_status_serializes_camel_case() {
        let json = serde_json::to_string(&engine_status()).unwrap();
        assert!(json.contains("\"engineVersion\""));
    }
}
