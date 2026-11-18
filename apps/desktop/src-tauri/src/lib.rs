use serde::Serialize;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![engine_status])
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
