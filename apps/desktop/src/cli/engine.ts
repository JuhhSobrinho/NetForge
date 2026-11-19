// Chamada ao motor para uma linha da CLI (Fase 4). O motor não guarda sessão: o modo vai junto.
import { invoke } from '@tauri-apps/api/core'
import type { Topology } from '../topology/model'

export type CliMode = 'user' | 'privileged' | 'config' | 'interface'

export interface CliSession {
  mode: CliMode
  interface?: string | null
}

export interface ArpEntry {
  address: string
  mac: string
  port: string
}

export interface MacEntry {
  mac: string
  port: string
}

/** Tabelas de um equipamento após um ping ou traceroute. */
export interface DeviceTables {
  deviceId: string
  arp: ArpEntry[]
  macTable: MacEntry[]
}

export interface CliOutcome {
  output: string[]
  session: CliSession
  prompt: string
  /** Presente somente quando a linha alterou a configuração. */
  topology: Topology | null
  error: boolean
  /** Presente quando a linha executou ping ou traceroute. */
  runtime: DeviceTables[] | null
}

export function cliExecute(
  topology: Topology,
  deviceId: string,
  session: CliSession,
  line: string,
  runtime: DeviceTables[] | null,
): Promise<CliOutcome> {
  return invoke<CliOutcome>('cli_execute', { topology, deviceId, session, line, runtime })
}
