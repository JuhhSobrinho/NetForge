// Abrir e salvar projetos. Diálogos do sistema via plugin; escrita/leitura via comandos Rust.
import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'

const FILTERS = [{ name: 'Projeto NetForge', extensions: ['json'] }]

export async function pickSavePath(suggestedName: string): Promise<string | null> {
  const path = await save({ defaultPath: suggestedName, filters: FILTERS })
  if (!path) return null
  return path.toLowerCase().endsWith('.json') ? path : `${path}.json`
}

export async function pickOpenPath(): Promise<string | null> {
  const result = await open({ multiple: false, directory: false, filters: FILTERS })
  return typeof result === 'string' ? result : null
}

export function saveProjectFile(path: string, contents: string): Promise<void> {
  return invoke<void>('save_project_file', { path, contents })
}

export function openProjectFile(path: string): Promise<string> {
  return invoke<string>('open_project_file', { path })
}
