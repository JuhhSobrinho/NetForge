// Chamadas ao motor de simulação. Os tipos espelham o JSON de netforge_engine::sim.
import { invoke } from '@tauri-apps/api/core'
import type { Topology } from '../topology/model'
import type { DeviceTables } from '../cli/engine'

export type LogKind = 'tx' | 'rx' | 'learn' | 'flood' | 'forward' | 'drop' | 'info'

export interface ArpInfo {
  op: string
  senderMac: string
  senderIp: string
  targetMac: string
  targetIp: string
}

export interface Ipv4Info {
  src: string
  dst: string
  ttl: number
  id: number
  protocol: number
}

export interface IcmpInfo {
  kind: string
  code: number
  id: number
  seq: number
}

/** Quadro decodificado por camada. */
export interface FrameInfo {
  protocol: string
  srcMac: string
  dstMac: string
  ethertype: number
  bytes: number
  arp?: ArpInfo | null
  ipv4?: Ipv4Info | null
  icmp?: IcmpInfo | null
}

/** Enlace percorrido por uma transmissão (equipamento e porta de cada ponta). */
export interface LinkRef {
  fromDevice: string
  fromPort: string
  toDevice: string
  toPort: string
}

export interface LogEntry {
  timeUs: number
  device: string
  deviceId: string
  kind: LogKind
  detail: string
  frame?: FrameInfo | null
  link?: LinkRef | null
}

export interface PingReply {
  seq: number
  rttUs: number
}

/** Conclusão do diagnóstico determinístico, com os índices do log que a sustentam. */
export interface Finding {
  severity: 'info' | 'warning' | 'error'
  title: string
  evidence: number[]
  suggestion: string
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
  /** Diagnóstico baseado nos eventos reais. */
  diagnostics: Finding[]
}

/** Enlace destacado no canvas enquanto a linha do tempo está nele. */
export interface ActiveLink extends LinkRef {
  protocol: string
}

/** Executa o ping no motor. Rejeita com uma mensagem legível se a entrada for inválida. */
export function runPing(topology: Topology, sourceDeviceId: string, targetIp: string, count: number): Promise<PingOutcome> {
  return invoke<PingOutcome>('run_ping', { topology, sourceDeviceId, targetIp, count })
}

export function exportPcap(path: string, bytes: number[]): Promise<void> {
  return invoke<void>('export_pcap', { path, bytes })
}
