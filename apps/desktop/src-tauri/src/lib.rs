mod project_io;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .invoke_handler(tauri::generate_handler![
        engine_status,
        save_project_file,
        open_project_file
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
