import { useCallback, useEffect, useState } from 'react'
import { TopologyCanvas, type Selection } from './canvas/TopologyCanvas'
import { getEngineStatus } from './ipc'
import { openProjectFile, pickOpenPath, pickSavePath, saveProjectFile } from './project/fileActions'
import { useTopologyHistory } from './topology/useTopologyHistory'
import {
  addDevice,
  connect,
  DEVICE_LABEL,
  emptyTopology,
  findDevice,
  linkOnPort,
  moveDevice,
  parseTopology,
  portKey,
  removeDevice,
  removeLink,
  renameDevice,
  serializeTopology,
  setPortAddress,
  type DeviceKind,
  type PortRef,
  type Topology,
} from './topology/model'
import { createStressTopology } from './topology/stress'
import { PingPanel } from './ping/PingPanel'
import type { DeviceTables } from './cli/engine'
import { DeviceTerminal } from './cli/DeviceTerminal'
import './App.css'

const newId = () => crypto.randomUUID()
const KINDS: DeviceKind[] = ['pc', 'switch', 'router', 'server']
const DEFAULT_FILE_NAME = 'projeto.netforge.json'

/** `?stress=N` (somente em DEV) carrega uma topologia sintética para medir o canvas. */
function initialTopology(): Topology {
  if (import.meta.env.DEV) {
    const n = Number(new URLSearchParams(window.location.search).get('stress'))
    if (n > 0) return createStressTopology(n, 300)
  }
  return emptyTopology()
}

type Message = { kind: 'ok' | 'error'; text: string }

export default function App() {
  const { topology, apply, undo, redo, canUndo, canRedo } = useTopologyHistory(initialTopology())
  const [selection, setSelection] = useState<Selection>({ deviceId: null, linkId: null })
  const [filePath, setFilePath] = useState<string | null>(null)
  const [message, setMessage] = useState<Message | null>(null)
  const [engineVersion, setEngineVersion] = useState<string | null>(null)
  // Tabelas ARP/MAC da última execução de ping ou traceroute (usadas por show arp e show mac).
  const [runtime, setRuntime] = useState<DeviceTables[] | null>(null)

  useEffect(() => {
    getEngineStatus()
      .then((s) => setEngineVersion(s.engineVersion))
      .catch(() => setEngineVersion(null))
  }, [])

  // Ctrl+Z / Ctrl+Y (ou Ctrl+Shift+Z), exceto quando o foco está em campo de texto.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null
      if (target?.closest('input, textarea, [contenteditable="true"]')) return
      const mod = e.ctrlKey || e.metaKey
      if (!mod) return
      const key = e.key.toLowerCase()
      if (key === 'z' && !e.shiftKey) {
        e.preventDefault()
        undo()
      } else if (key === 'y' || (key === 'z' && e.shiftKey)) {
        e.preventDefault()
        redo()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [undo, redo])

  const fail = (text: string) => setMessage({ kind: 'error', text })
  const ok = (text: string) => setMessage({ kind: 'ok', text })

  const addEquipment = (kind: DeviceKind) => {
    const n = topology.devices.length
    const position = { x: 60 + (n % 6) * 200, y: 60 + Math.floor(n / 6) * 160 }
    apply((t) => addDevice(t, kind, position, newId))
    setMessage(null)
  }

  const moveDevices = useCallback(
    (moves: { id: string; x: number; y: number }[]) => {
      const changed = moves.some((m) => {
        const d = findDevice(topology, m.id)
        return !d || d.x !== m.x || d.y !== m.y
      })
      if (!changed) return
      apply((t) => moves.reduce((acc, m) => moveDevice(acc, m.id, m.x, m.y), t))
    },
    [topology, apply],
  )

  const connectPorts = useCallback(
    (a: PortRef, b: PortRef) => {
      const result = connect(topology, a, b, newId)
      if (!result.ok) return fail(result.error)
      apply(() => result.value)
      ok('Ligação criada.')
    },
    [topology, apply],
  )

  const deleteElements = useCallback(
    (deviceIds: string[], linkIds: string[]) => {
      if (deviceIds.length === 0 && linkIds.length === 0) return
      apply((t) => {
        let next = t
        for (const id of deviceIds) next = removeDevice(next, id)
        for (const id of linkIds) next = removeLink(next, id)
        return next
      })
      setSelection({ deviceId: null, linkId: null })
    },
    [apply],
  )

  const rename = (id: string, name: string) => {
    const result = renameDevice(topology, id, name)
    if (!result.ok) return fail(result.error)
    apply(() => result.value)
    ok('Nome atualizado.')
  }

  const setAddress = (deviceId: string, portId: string, ip: string, prefixLen: number | undefined) => {
    const result = setPortAddress(topology, deviceId, portId, ip, prefixLen)
    if (!result.ok) return fail(result.error)
    apply(() => result.value)
    ok(ip.trim() === '' ? 'Endereço removido.' : 'Endereço atualizado.')
  }

  const newProject = () => {
    if (topology.devices.length > 0 && !window.confirm('Criar um novo projeto? O projeto atual não está salvo.'))
      return
    apply(() => emptyTopology())
    setFilePath(null)
    setSelection({ deviceId: null, linkId: null })
    ok('Novo projeto.')
  }

  const openProject = async () => {
    try {
      const path = await pickOpenPath()
      if (!path) return
      const json = await openProjectFile(path)
      const parsed = parseTopology(json)
      if (!parsed.ok) return fail(parsed.error)
      apply(() => parsed.value)
      setFilePath(path)
      setSelection({ deviceId: null, linkId: null })
      ok('Projeto aberto.')
    } catch (e) {
      fail(`Não foi possível abrir: ${String(e)}`)
    }
  }

  const saveProject = async () => {
    try {
      const path = filePath ?? (await pickSavePath(DEFAULT_FILE_NAME))
      if (!path) return
      await saveProjectFile(path, serializeTopology(topology))
      setFilePath(path)
      ok(`Projeto salvo em ${path}`)
    } catch (e) {
      fail(`Não foi possível salvar: ${String(e)}`)
    }
  }

  const device = selection.deviceId ? findDevice(topology, selection.deviceId) : undefined
  const link = selection.linkId ? topology.links.find((l) => l.id === selection.linkId) : undefined

  return (
    <div className="app">
      <header className="toolbar">
        <div className="toolbar__group">
          <span className="toolbar__title">NetForge</span>
          <button onClick={newProject}>Novo</button>
          <button onClick={openProject}>Abrir</button>
          <button onClick={saveProject}>Salvar</button>
        </div>
        <div className="toolbar__group">
          <button onClick={undo} disabled={!canUndo} title="Desfazer (Ctrl+Z)">
            Desfazer
          </button>
          <button onClick={redo} disabled={!canRedo} title="Refazer (Ctrl+Y)">
            Refazer
          </button>
        </div>
        <div className="toolbar__group">
          <span className="muted">Adicionar:</span>
          {KINDS.map((kind) => (
            <button key={kind} onClick={() => addEquipment(kind)}>
              {DEVICE_LABEL[kind]}
            </button>
          ))}
        </div>
      </header>

      <div className="workspace">
        <TopologyCanvas
          topology={topology}
          selection={selection}
          onSelectionChange={setSelection}
          onMoveDevices={moveDevices}
          onConnectPorts={connectPorts}
          onDelete={deleteElements}
        />

        <aside className="properties" aria-label="Propriedades">
          {device ? (
            <DeviceProperties
              key={device.id}
              device={device}
              topology={topology}
              onRename={rename}
              onSetAddress={setAddress}
              onConfigure={(t) => apply(() => t)}
              runtime={runtime}
              onRuntime={setRuntime}
              onRemove={() => deleteElements([device.id], [])}
            />
          ) : link ? (
            <LinkProperties
              topology={topology}
              linkId={link.id}
              onRemove={() => deleteElements([], [link.id])}
            />
          ) : (
            <>
              <h2>Propriedades</h2>
              <p className="muted">
                {topology.devices.length === 0
                  ? 'Adicione equipamentos pela barra superior e arraste de uma porta para outra para ligá-los.'
                  : 'Selecione um equipamento ou um link.'}
              </p>
            </>
          )}
          <PingPanel topology={topology} onRuntime={setRuntime} />
        </aside>
      </div>

      <footer className="status" role="status">
        {message && <span className={message.kind === 'error' ? 'status--error' : 'status--ok'}>{message.text}</span>}
        <span className="muted" style={{ float: 'right' }}>
          {engineVersion ? `Motor ${engineVersion}` : ''}
        </span>
      </footer>
    </div>
  )
}

function DeviceProperties({
  device,
  topology,
  onRename,
  onSetAddress,
  onConfigure,
  runtime,
  onRuntime,
  onRemove,
}: {
  device: NonNullable<ReturnType<typeof findDevice>>
  topology: Topology
  onRename: (id: string, name: string) => void
  onSetAddress: (deviceId: string, portId: string, ip: string, prefixLen: number | undefined) => void
  onConfigure: (t: Topology) => void
  runtime: DeviceTables[] | null
  onRuntime: (t: DeviceTables[]) => void
  onRemove: () => void
}) {
  const [draft, setDraft] = useState(device.name)

  const commitName = () => {
    if (draft.trim() !== device.name) onRename(device.id, draft)
  }

  return (
    <>
      <h2>{DEVICE_LABEL[device.kind]}</h2>
      <label htmlFor="device-name">Nome</label>
      <input
        id="device-name"
        value={draft}
        maxLength={64}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commitName}
        onKeyDown={(e) => {
          if (e.key === 'Enter') (e.target as HTMLInputElement).blur()
        }}
      />
      <h3 style={{ fontSize: 13, margin: '16px 0 4px' }}>Portas</h3>
      <ul>
        {device.ports.map((port) => {
          const ref = { deviceId: device.id, portId: port.id }
          const l = linkOnPort(topology, ref)
          let status = 'livre'
          if (l) {
            const other = portKey(l.from) === portKey(ref) ? l.to : l.from
            const otherDevice = findDevice(topology, other.deviceId)
            status = `ligada a ${otherDevice?.name ?? '?'} · ${other.portId}`
          }
          if (port.shutdown) status = `desligada · ${status}`
          return (
            <li key={port.id} className="port-row">
              <div className="port-row__head">
                <span>{port.name}</span>
                <span className="muted">{status}</span>
              </div>
              <PortAddressEditor
                key={`${device.id}:${port.id}:${port.ip ?? ''}:${port.prefixLen ?? ''}`}
                deviceId={device.id}
                portId={port.id}
                ip={port.ip}
                prefixLen={port.prefixLen}
                onSet={onSetAddress}
              />
            </li>
          )
        })}
      </ul>
      <DeviceTerminal
        key={device.id}
        topology={topology}
        device={device}
        runtime={runtime}
        onTopology={onConfigure}
        onRuntime={onRuntime}
      />
      <div className="actions">
        <button className="danger" onClick={onRemove}>
          Remover equipamento
        </button>
      </div>
    </>
  )
}

function PortAddressEditor({
  deviceId,
  portId,
  ip,
  prefixLen,
  onSet,
}: {
  deviceId: string
  portId: string
  ip: string | undefined
  prefixLen: number | undefined
  onSet: (deviceId: string, portId: string, ip: string, prefixLen: number | undefined) => void
}) {
  const [ipDraft, setIpDraft] = useState(ip ?? '')
  const [prefixDraft, setPrefixDraft] = useState(prefixLen === undefined ? '' : String(prefixLen))

  const commit = () => {
    const trimmedIp = ipDraft.trim()
    const trimmedPrefix = prefixDraft.trim()
    const prefix = trimmedPrefix === '' ? undefined : Number(trimmedPrefix)
    if (trimmedIp === (ip ?? '') && prefix === prefixLen) return
    onSet(deviceId, portId, trimmedIp, prefix)
  }

  return (
    <div className="port-row__address">
      <input
        aria-label={`Endereço IPv4 da ${portId}`}
        placeholder="IPv4, ex.: 10.0.0.1"
        value={ipDraft}
        onChange={(e) => setIpDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') (e.target as HTMLInputElement).blur()
        }}
      />
      <input
        aria-label={`Prefixo da ${portId}`}
        className="port-row__prefix"
        placeholder="/24"
        inputMode="numeric"
        value={prefixDraft}
        onChange={(e) => setPrefixDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') (e.target as HTMLInputElement).blur()
        }}
      />
    </div>
  )
}

function LinkProperties({
  topology,
  linkId,
  onRemove,
}: {
  topology: Topology
  linkId: string
  onRemove: () => void
}) {
  const l = topology.links.find((x) => x.id === linkId)
  if (!l) return null
  const describe = (ref: PortRef) => {
    const d = findDevice(topology, ref.deviceId)
    return `${d?.name ?? '?'} · ${ref.portId}`
  }
  return (
    <>
      <h2>Link</h2>
      <ul>
        <li>
          <span>Origem</span>
          <span>{describe(l.from)}</span>
        </li>
        <li>
          <span>Destino</span>
          <span>{describe(l.to)}</span>
        </li>
      </ul>
      <div className="actions">
        <button className="danger" onClick={onRemove}>
          Remover link
        </button>
      </div>
    </>
  )
}
