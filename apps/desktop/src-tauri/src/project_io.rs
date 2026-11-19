//! Leitura e escrita de arquivos do usuário: projetos (JSON) e capturas (PCAP).
//!
//! Não interpreta o conteúdo do projeto: só garante caminho absoluto, extensão e tamanho.
//! A validação da topologia fica no motor (netforge_engine::topology).

use std::fs;
use std::path::{Path, PathBuf};

/// Limite de tamanho de arquivo de projeto (5 MiB).
pub const MAX_PROJECT_BYTES: u64 = 5 * 1024 * 1024;
/// Limite de tamanho de captura exportada (16 MiB).
pub const MAX_EXPORT_BYTES: usize = 16 * 1024 * 1024;

/// Valida o caminho escolhido pelo usuário: caminho absoluto e a extensão esperada.
pub fn validate_path_with(raw: &str, expected_ext: &str) -> Result<PathBuf, String> {
    if raw.trim().is_empty() {
        return Err("Caminho vazio.".into());
    }
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err("O caminho precisa ser absoluto.".into());
    }
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case(expected_ext) => Ok(path),
        _ => Err(format!("O arquivo precisa ter extensão .{expected_ext}.")),
    }
}

/// Valida o caminho de um projeto: extensão `.json` e caminho absoluto.
pub fn validate_project_path(raw: &str) -> Result<PathBuf, String> {
    validate_path_with(raw, "json")
}

/// Grava uma captura PCAP gerada pelo motor.
pub fn write_capture(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_EXPORT_BYTES {
        return Err("Captura maior que o limite de 16 MiB.".into());
    }
    fs::write(path, bytes).map_err(|e| format!("Não foi possível salvar a captura: {e}"))
}

pub fn read_project(path: &Path) -> Result<String, String> {
    let meta = fs::metadata(path).map_err(|e| format!("Não foi possível ler o arquivo: {e}"))?;
    if meta.len() > MAX_PROJECT_BYTES {
        return Err("Arquivo de projeto maior que o limite de 5 MiB.".into());
    }
    fs::read_to_string(path).map_err(|e| format!("Não foi possível ler o arquivo: {e}"))
}

pub fn write_project(path: &Path, contents: &str) -> Result<(), String> {
    if contents.len() as u64 > MAX_PROJECT_BYTES {
        return Err("Projeto maior que o limite de 5 MiB.".into());
    }
    fs::write(path, contents).map_err(|e| format!("Não foi possível salvar o arquivo: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_absolute_json_paths() {
        let p = std::env::temp_dir().join("netforge-test.json");
        assert!(validate_project_path(p.to_str().unwrap()).is_ok());
    }

    #[test]
    fn rejects_other_extensions_and_relative_paths() {
        let p = std::env::temp_dir().join("x.exe");
        assert!(validate_project_path(p.to_str().unwrap()).is_err());
        assert!(validate_project_path("relative/file.json").is_err());
        assert!(validate_project_path("   ").is_err());
    }

    #[test]
    fn write_then_read_round_trips() {
        let p = std::env::temp_dir().join("netforge-roundtrip-test.json");
        write_project(&p, "{\"schemaVersion\":1}\n").unwrap();
        assert_eq!(read_project(&p).unwrap(), "{\"schemaVersion\":1}\n");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn rejects_oversized_writes() {
        let p = std::env::temp_dir().join("netforge-big-test.json");
        let big = "a".repeat((MAX_PROJECT_BYTES + 1) as usize);
        assert!(write_project(&p, &big).is_err());
    }

    #[test]
    fn capture_paths_require_pcap_extension() {
        let p = std::env::temp_dir().join("captura.pcap");
        assert!(validate_path_with(p.to_str().unwrap(), "pcap").is_ok());
        assert!(validate_path_with(p.to_str().unwrap(), "json").is_err());
    }

    #[test]
    fn write_capture_writes_bytes() {
        let p = std::env::temp_dir().join("netforge-capture-test.pcap");
        write_capture(&p, &[0xd4, 0xc3, 0xb2, 0xa1]).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), vec![0xd4, 0xc3, 0xb2, 0xa1]);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn reading_missing_file_reports_error() {
        let p = std::env::temp_dir().join("netforge-does-not-exist-xyz.json");
        assert!(read_project(&p).is_err());
    }
}
