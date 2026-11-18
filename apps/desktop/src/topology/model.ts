// Modelo de topologia (Fase 2). Funções puras: nunca mutam o argumento.
// Regras de rede (protocolos, encaminhamento) NÃO pertencem a este módulo.

export const SCHEMA_VERSION = 1 as const
export const MAX_NAME_LENGTH = 32
export const MAX_FILE_DEVICES = 1000

export type DeviceKind = 'pc' | 'switch' | 'router' | 'server'

export interface Port {
  id: string
  name: string
}

export interface Device {
  id: string
  kind: DeviceKind
  name: string
  x: number
  y: number
  ports: Port[]
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
    }
    if (typeof d.x !== 'number' || typeof d.y !== 'number') return fail('Posição de equipamento inválida.')
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
