// Chamada ao motor para uma linha da CLI (Fase 4). O motor não guarda sessão: o modo vai junto.
import { invoke } from '@tauri-apps/api/core'
import type { Topology } from '../topology/model'

export type CliMode = 'user' | 'privileged' | 'config' | 'interface'

export interface CliSession {
  mode: CliMode
  interface?: string | null
}

export interface CliOutcome {
  output: string[]
  session: CliSession
  prompt: string
  /** Presente somente quando a linha alterou a configuração. */
  topology: Topology | null
  error: boolean
}

export function cliExecute(topology: Topology, deviceId: string, session: CliSession, line: string): Promise<CliOutcome> {
  return invoke<CliOutcome>('cli_execute', { topology, deviceId, session, line })
}
