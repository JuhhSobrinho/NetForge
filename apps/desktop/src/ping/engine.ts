// Chamadas ao motor de simulação (Fase 3). Os tipos espelham o JSON de netforge_engine::sim.
import { invoke } from '@tauri-apps/api/core'
import type { Topology } from '../topology/model'
import type { DeviceTables } from '../cli/engine'

export type LogKind = 'tx' | 'rx' | 'learn' | 'flood' | 'forward' | 'drop' | 'info'

export interface LogEntry {
  timeUs: number
  device: string
  kind: LogKind
  detail: string
}

export interface PingReply {
  seq: number
  rttUs: number
}

export interface PingOutcome {
  target: string
  transmitted: number
  replies: PingReply[]
  error: string | null
  log: LogEntry[]
  logTruncated: boolean
  /** Bytes do arquivo PCAP (cabeçalho global + quadros da interface de origem). */
  pcap: number[]
  /** Tabelas ARP e MAC ao final da execução. */
  tables: DeviceTables[]
}

/** Executa o ping no motor. Rejeita com uma mensagem legível se a entrada for inválida. */
export function runPing(topology: Topology, sourceDeviceId: string, targetIp: string, count: number): Promise<PingOutcome> {
  return invoke<PingOutcome>('run_ping', { topology, sourceDeviceId, targetIp, count })
}

export function exportPcap(path: string, bytes: number[]): Promise<void> {
  return invoke<void>('export_pcap', { path, bytes })
}
