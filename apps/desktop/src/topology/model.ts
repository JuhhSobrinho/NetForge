// Modelo de topologia (Fase 2). Funções puras: nunca mutam o argumento.
// Regras de rede (protocolos, encaminhamento) NÃO pertencem a este módulo.

export const SCHEMA_VERSION = 1 as const
export const MAX_NAME_LENGTH = 32
export const MAX_FILE_DEVICES = 1000

export type DeviceKind = 'pc' | 'switch' | 'router' | 'server'

export interface Port {
  id: string
  name: string
  /** Endereço IPv4 da interface (Fase 3). Ausente = sem endereço. */
  ip?: string
  /** Comprimento do prefixo (0–32). Só existe junto com `ip`. */
  prefixLen?: number
  /** Interface administrativamente desligada (`shutdown` na CLI). */
  shutdown?: boolean
  /** Modo VLAN de uma porta de switch. */
  vlanMode?: 'access' | 'trunk'
  /** VLAN de acesso (1–4094). */
  vlan?: number
}

/** Registro DNS servido por um equipamento (`ip dns-record`). */
export interface DnsRecord {
  name: string
  ip: string
}

/** Pool de um servidor DHCP (`ip dhcp-pool`). */
export interface DhcpPool {
  network: string
  prefixLen: number
  start: string
  end: string
  gateway: string
}

/** Rota estática de um roteador (`ip route`). */
export interface StaticRoute {
  network: string
  prefixLen: number
  nextHop: string
}

const IPV4_RE = /^(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}$/

export function isValidIpv4(s: string): boolean {
  return IPV4_RE.test(s)
}

/** Valida o par (ip, prefixo). Os dois campos vêm juntos ou não vêm. */
export function validateAddress(ip: string | undefined, prefixLen: number | undefined): Result<true> {
  if (ip === undefined && prefixLen === undefined) return ok(true)
  if (ip === undefined || prefixLen === undefined) return fail('Endereço e máscara devem ser informados juntos.')
  if (!isValidIpv4(ip)) return fail(`Endereço IPv4 inválido: ${ip}.`)
  if (!Number.isInteger(prefixLen) || prefixLen < 0 || prefixLen > 32)
    return fail('O prefixo deve ser um inteiro entre 0 e 32.')
  return ok(true)
}

/** Define (ou remove, com ip vazio) o endereço de uma porta. */
export function setPortAddress(
  t: Topology,
  deviceId: string,
  portId: string,
  ip: string,
  prefixLen: number | undefined,
): Result<Topology> {
  const device = findDevice(t, deviceId)
  if (!device) return fail('Equipamento não encontrado.')
  if (!device.ports.some((p) => p.id === portId)) return fail('Porta não encontrada.')
  const trimmed = ip.trim()
  const address = trimmed === '' ? { ip: undefined, prefixLen: undefined } : { ip: trimmed, prefixLen }
  const check = validateAddress(address.ip, address.prefixLen)
  if (!check.ok) return check
  return ok({
    ...t,
    devices: t.devices.map((d) =>
      d.id !== deviceId
        ? d
        : {
            ...d,
            ports: d.ports.map((p) => {
              if (p.id !== portId) return p
              const { ip: _ip, prefixLen: _p, ...rest } = p
              void _ip
              void _p
              return address.ip === undefined ? rest : { ...rest, ip: address.ip, prefixLen: address.prefixLen }
            }),
          },
    ),
  })
}

export interface Device {
  id: string
  kind: DeviceKind
  name: string
  x: number
  y: number
  ports: Port[]
  /** Gateway padrão de PCs e servidores (`ip default-gateway`). */
  gateway?: string
  /** Rotas estáticas de roteadores. */
  routes?: StaticRoute[]
  /** Pool DHCP de um roteador ou servidor. */
  dhcpPool?: DhcpPool
  /** Servidor de nomes de um PC ou servidor (`ip name-server`). */
  nameServer?: string
  /** Registros DNS servidos pelo equipamento (`ip dns-record`). */
  dnsRecords?: DnsRecord[]
  /** Portas TCP em escuta (`ip tcp-listen`). */
  tcpListen?: number[]
  /** RIPv2 ativo no roteador (`router rip`). */
  rip?: boolean
}

export interface PortRef {
  deviceId: string
  portId: string
}

export interface Link {
  id: string
  from: PortRef
  to: PortRef
}

export interface Topology {
  schemaVersion: typeof SCHEMA_VERSION
  devices: Device[]
  links: Link[]
}

export type Result<T> = { ok: true; value: T } | { ok: false; error: string }

const ok = <T>(value: T): Result<T> => ({ ok: true, value })
const fail = <T = never>(error: string): Result<T> => ({ ok: false, error })

/** Quantidade de portas por tipo de equipamento (valores didáticos, ver FEATURES.md §2). */
export const PORT_COUNT: Record<DeviceKind, number> = {
  pc: 1,
  switch: 8,
  router: 4,
  server: 1,
}

export const DEVICE_LABEL: Record<DeviceKind, string> = {
  pc: 'PC',
  switch: 'Switch',
  router: 'Roteador',
  server: 'Servidor',
}

export function emptyTopology(): Topology {
  return { schemaVersion: SCHEMA_VERSION, devices: [], links: [] }
}

export function makePorts(kind: DeviceKind): Port[] {
  return Array.from({ length: PORT_COUNT[kind] }, (_, i) => ({
    id: `p${i + 1}`,
    name: `Porta ${i + 1}`,
  }))
}

export function validateName(name: string): Result<string> {
  const trimmed = name.trim()
  if (trimmed.length === 0) return fail('O nome não pode ficar vazio.')
  if (trimmed.length > MAX_NAME_LENGTH)
    return fail(`O nome pode ter no máximo ${MAX_NAME_LENGTH} caracteres.`)
  return ok(trimmed)
}

export function findDevice(t: Topology, id: string): Device | undefined {
  return t.devices.find((d) => d.id === id)
}

export function portKey(ref: PortRef): string {
  return `${ref.deviceId}:${ref.portId}`
}

/** Link que ocupa a porta, se houver. */
export function linkOnPort(t: Topology, ref: PortRef): Link | undefined {
  const key = portKey(ref)
  return t.links.find((l) => portKey(l.from) === key || portKey(l.to) === key)
}

export function addDevice(
  t: Topology,
  kind: DeviceKind,
  position: { x: number; y: number },
  idFactory: () => string,
): Topology {
  const number = t.devices.filter((d) => d.kind === kind).length + 1
  const device: Device = {
    id: idFactory(),
    kind,
    name: `${DEVICE_LABEL[kind]} ${number}`,
    x: position.x,
    y: position.y,
    ports: makePorts(kind),
  }
  return { ...t, devices: [...t.devices, device] }
}

export function renameDevice(t: Topology, id: string, name: string): Result<Topology> {
  const valid = validateName(name)
  if (!valid.ok) return valid
  if (!findDevice(t, id)) return fail('Equipamento não encontrado.')
  return ok({
    ...t,
    devices: t.devices.map((d) => (d.id === id ? { ...d, name: valid.value } : d)),
  })
}

export function moveDevice(t: Topology, id: string, x: number, y: number): Topology {
  return {
    ...t,
    devices: t.devices.map((d) => (d.id === id ? { ...d, x, y } : d)),
  }
}

export function removeDevice(t: Topology, id: string): Topology {
  return {
    ...t,
    devices: t.devices.filter((d) => d.id !== id),
    links: t.links.filter((l) => l.from.deviceId !== id && l.to.deviceId !== id),
  }
}

export function removeLink(t: Topology, id: string): Topology {
  return { ...t, links: t.links.filter((l) => l.id !== id) }
}

/** Valida um endpoint: dispositivo e porta existem e a porta está livre. */
function checkFreeEndpoint(t: Topology, ref: PortRef): Result<true> {
  const device = findDevice(t, ref.deviceId)
  if (!device) return fail('Equipamento de origem/destino não existe.')
  if (!device.ports.some((p) => p.id === ref.portId)) return fail('Porta não existe neste equipamento.')
  if (linkOnPort(t, ref)) return fail(`A porta de ${device.name} já está conectada.`)
  return ok(true)
}

export function connect(
  t: Topology,
  a: PortRef,
  b: PortRef,
  idFactory: () => string,
): Result<Topology> {
  if (a.deviceId === b.deviceId) return fail('Não é possível conectar um equipamento a ele mesmo.')
  if (portKey(a) === portKey(b)) return fail('As duas pontas são a mesma porta.')
  const first = checkFreeEndpoint(t, a)
  if (!first.ok) return first
  const second = checkFreeEndpoint(t, b)
  if (!second.ok) return second
  return ok({ ...t, links: [...t.links, { id: idFactory(), from: a, to: b }] })
}

/**
 * Lê e valida um JSON de projeto. Rejeita versões desconhecidas, ids duplicados,
 * links apontando para portas inexistentes e portas usadas mais de uma vez.
 */
export function parseTopology(json: string): Result<Topology> {
  let raw: unknown
  try {
    raw = JSON.parse(json)
  } catch {
    return fail('Arquivo não é um JSON válido.')
  }
  if (typeof raw !== 'object' || raw === null) return fail('Formato de projeto inválido.')
  const obj = raw as Record<string, unknown>
  if (obj.schemaVersion !== SCHEMA_VERSION)
    return fail(`Versão de projeto não suportada: ${String(obj.schemaVersion)}.`)
  if (!Array.isArray(obj.devices) || !Array.isArray(obj.links))
    return fail('Projeto sem listas de equipamentos ou links.')
  if (obj.devices.length > MAX_FILE_DEVICES) return fail('Projeto excede o limite de equipamentos.')

  const devices = obj.devices as Device[]
  const deviceIds = new Set<string>()
  for (const d of devices) {
    if (!d || typeof d.id !== 'string' || deviceIds.has(d.id))
      return fail('Equipamento com id inválido ou duplicado.')
    if (!(d.kind in PORT_COUNT)) return fail(`Tipo de equipamento desconhecido: ${String(d.kind)}.`)
    const nameCheck = validateName(String(d.name ?? ''))
    if (!nameCheck.ok) return fail(nameCheck.error)
    if (!Array.isArray(d.ports)) return fail('Equipamento sem lista de portas.')
    const portIds = new Set<string>()
    for (const p of d.ports) {
      if (!p || typeof p.id !== 'string' || portIds.has(p.id))
        return fail('Porta com id inválido ou duplicado.')
      portIds.add(p.id)
      const addr = validateAddress(p.ip, p.prefixLen)
      if (!addr.ok) return fail(addr.error)
      if (p.shutdown !== undefined && typeof p.shutdown !== 'boolean') return fail('Estado de shutdown inválido.')
      if (p.vlanMode !== undefined || p.vlan !== undefined) {
        if (d.kind !== 'switch') return fail('Configuração de VLAN só existe em switches.')
        if (p.vlanMode !== undefined && p.vlanMode !== 'access' && p.vlanMode !== 'trunk') return fail('Modo de VLAN inválido.')
        if (p.vlan !== undefined && (!Number.isInteger(p.vlan) || p.vlan < 1 || p.vlan > 4094)) return fail('VLAN fora de 1–4094.')
        if (p.vlanMode === 'trunk' && p.vlan !== undefined) return fail('Porta tronco não tem VLAN de acesso.')
      }
    }
    if (typeof d.x !== 'number' || typeof d.y !== 'number') return fail('Posição de equipamento inválida.')
    if (d.gateway !== undefined) {
      if (d.kind !== 'pc' && d.kind !== 'server') return fail('Gateway padrão só existe em PCs e servidores.')
      if (!isValidIpv4(String(d.gateway))) return fail(`Gateway inválido: ${String(d.gateway)}.`)
    }
    if (d.dhcpPool !== undefined) {
      const p = d.dhcpPool
      if (d.kind === 'switch') return fail('Pool DHCP não existe em switches.')
      if (!isValidIpv4(String(p?.network)) || !isValidIpv4(String(p?.start)) || !isValidIpv4(String(p?.end)) || !isValidIpv4(String(p?.gateway)))
        return fail('Pool DHCP com endereço inválido.')
      if (!Number.isInteger(p.prefixLen) || p.prefixLen < 0 || p.prefixLen > 32) return fail('Prefixo do pool DHCP inválido.')
    }
    if (d.tcpListen !== undefined) {
      if (!Array.isArray(d.tcpListen)) return fail('Portas TCP inválidas.')
      if (d.tcpListen.length > 0 && d.kind === 'switch') return fail('Portas TCP não existem em switches.')
      for (const p of d.tcpListen) {
        if (!Number.isInteger(p) || p < 1 || p > 65535) return fail('Porta TCP fora de 1–65535.')
      }
    }
    if (d.rip !== undefined) {
      if (typeof d.rip !== 'boolean') return fail('Estado de RIP inválido.')
      if (d.rip && d.kind !== 'router') return fail('RIP só existe em roteadores.')
    }
    if (d.nameServer !== undefined) {
      if (d.kind !== 'pc' && d.kind !== 'server') return fail('Servidor de nomes só existe em PCs e servidores.')
      if (!isValidIpv4(String(d.nameServer))) return fail(`Servidor de nomes inválido: ${String(d.nameServer)}.`)
    }
    if (d.dnsRecords !== undefined) {
      if (!Array.isArray(d.dnsRecords)) return fail('Registros DNS inválidos.')
      if (d.dnsRecords.length > 0 && d.kind === 'switch') return fail('Registros DNS não existem em switches.')
      for (const r of d.dnsRecords) {
        if (typeof r?.name !== 'string' || !/^[a-z0-9-]+(\.[a-z0-9-]+)*$/.test(r.name.toLowerCase()) || !isValidIpv4(String(r.ip)))
          return fail('Registro DNS inválido.')
      }
    }
    if (d.routes !== undefined) {
      if (!Array.isArray(d.routes)) return fail('Rotas inválidas.')
      if (d.routes.length > 0 && d.kind !== 'router') return fail('Rotas estáticas só existem em roteadores.')
      for (const r of d.routes) {
        if (!isValidIpv4(String(r?.network)) || !isValidIpv4(String(r?.nextHop)) || !Number.isInteger(r?.prefixLen) || r.prefixLen < 0 || r.prefixLen > 32)
          return fail('Rota estática inválida.')
      }
    }
    deviceIds.add(d.id)
  }

  const candidate: Topology = { schemaVersion: SCHEMA_VERSION, devices, links: [] }
  const linkIds = new Set<string>()
  const used = new Set<string>()
  for (const l of obj.links as Link[]) {
    if (!l || typeof l.id !== 'string' || linkIds.has(l.id))
      return fail('Link com id inválido ou duplicado.')
    linkIds.add(l.id)
    for (const ref of [l.from, l.to]) {
      const device = findDevice(candidate, ref?.deviceId)
      if (!device) return fail('Link aponta para equipamento inexistente.')
      if (!device.ports.some((p) => p.id === ref.portId)) return fail('Link aponta para porta inexistente.')
      const key = portKey(ref)
      if (used.has(key)) return fail('Uma porta aparece em mais de um link.')
      used.add(key)
    }
    candidate.links.push(l)
  }
  return ok(candidate)
}

export function serializeTopology(t: Topology): string {
  return JSON.stringify(t, null, 2) + '\n'
}
