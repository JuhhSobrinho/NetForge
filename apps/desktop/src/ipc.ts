import { invoke } from '@tauri-apps/api/core'

/** Espelha `EngineStatus` em src-tauri/src/lib.rs (serde camelCase). */
export interface EngineStatus {
  engineVersion: string
}

/** Único ponto do frontend que chama comandos do backend. */
export function getEngineStatus(): Promise<EngineStatus> {
  return invoke<EngineStatus>('engine_status')
}
