import { useEffect, useMemo, useState, type ReactElement } from 'react'
import type { ActiveLink, FrameInfo, LogEntry, LogKind } from '../ping/engine'

const KIND_LABEL: Record<LogKind, string> = {
  tx: 'TX',
  rx: 'RX',
  learn: 'aprende',
  flood: 'inunda',
  forward: 'encaminha',
  drop: 'descarta',
  info: 'info',
}

const PROTOCOLS = ['todos', 'ARP', 'ICMP', 'IPv4', 'Ethernet'] as const
type ProtocolFilter = (typeof PROTOCOLS)[number]
const SPEEDS = [0.5, 1, 2, 4] as const
/** Intervalo entre eventos a 1×, em milissegundos. */
const BASE_DELAY_MS = 600
const MAX_ROWS = 400

/**
 * Linha do tempo de uma execução. O histórico completo está no log; "voltar" move o cursor
 * dentro dele (equivalente a reexibir o estado anterior, pois o motor é determinístico).
 */
export function Timeline({
  entries,
  names,
  onActiveLink,
}: {
  entries: LogEntry[]
  names: Record<string, string>
  onActiveLink: (link: ActiveLink | null) => void
}) {
  const [cursor, setCursor] = useState(-1)
  const [playing, setPlaying] = useState(false)
  const [speed, setSpeed] = useState<(typeof SPEEDS)[number]>(1)
  const [filter, setFilter] = useState<ProtocolFilter>('todos')

  // Índices dos eventos que passam pelo filtro de protocolo.
  const visible = useMemo(
    () =>
      entries
        .map((e, i) => ({ e, i }))
        .filter(({ e }) => filter === 'todos' || e.frame?.protocol === filter)
        .map(({ i }) => i),
    [entries, filter],
  )

  const current = cursor >= 0 ? entries[cursor] : undefined

  // Destaca o enlace da transmissão atual no canvas.
  useEffect(() => {
    if (current?.link && current.frame) {
      onActiveLink({ ...current.link, protocol: current.frame.protocol })
    } else {
      onActiveLink(null)
    }
  }, [current, onActiveLink])

  useEffect(() => () => onActiveLink(null), [onActiveLink])

  // Reprodução automática.
  useEffect(() => {
    if (!playing) return
    const id = window.setInterval(() => {
      setCursor((c) => {
        const next = visible.find((i) => i > c)
        if (next === undefined) {
          setPlaying(false)
          return c
        }
        return next
      })
    }, BASE_DELAY_MS / speed)
    return () => window.clearInterval(id)
  }, [playing, speed, visible])

  const stepForward = () => {
    setPlaying(false)
    const next = visible.find((i) => i > cursor)
    if (next !== undefined) setCursor(next)
  }
  const stepBack = () => {
    setPlaying(false)
    const prev = [...visible].reverse().find((i) => i < cursor)
    if (prev !== undefined) setCursor(prev)
    else setCursor(-1)
  }
  const restart = () => {
    setPlaying(false)
    setCursor(-1)
  }

  const rows = visible.slice(0, MAX_ROWS)
  const atEnd = visible.length === 0 || cursor >= visible[visible.length - 1]

  return (
    <section className="timeline" aria-label="Linha do tempo">
      <div className="timeline__controls">
        <button onClick={restart} disabled={cursor < 0}>
          ⏮ Início
        </button>
        <button onClick={stepBack} disabled={cursor < 0}>
          ◀ Voltar
        </button>
        <button onClick={() => setPlaying((p) => !p)} disabled={atEnd && !playing}>
          {playing ? '❚❚ Pausar' : '▶ Reproduzir'}
        </button>
        <button onClick={stepForward} disabled={atEnd}>
          Avançar ▶
        </button>
      </div>
      <div className="timeline__controls">
        <label>
          Velocidade{' '}
          <select value={speed} onChange={(e) => setSpeed(Number(e.target.value) as (typeof SPEEDS)[number])}>
            {SPEEDS.map((s) => (
              <option key={s} value={s}>
                {s}×
              </option>
            ))}
          </select>
        </label>
        <label>
          Protocolo{' '}
          <select value={filter} onChange={(e) => setFilter(e.target.value as ProtocolFilter)}>
            {PROTOCOLS.map((p) => (
              <option key={p} value={p}>
                {p}
              </option>
            ))}
          </select>
        </label>
      </div>
      <input
        className="timeline__scrub"
        type="range"
        aria-label="Posição na linha do tempo"
        min={-1}
        max={Math.max(entries.length - 1, 0)}
        value={cursor}
        onChange={(e) => {
          setPlaying(false)
          setCursor(Number(e.target.value))
        }}
      />

      <ol className="timeline__list" aria-label="Eventos">
        {rows.map((i) => {
          const e = entries[i]
          return (
            <li key={i}>
              <button
                className={`timeline__row${i === cursor ? ' timeline__row--current' : ''}`}
                onClick={() => {
                  setPlaying(false)
                  setCursor(i)
                }}
              >
                <span className="muted">{(e.timeUs / 1000).toFixed(2)} ms</span>
                <strong>{KIND_LABEL[e.kind]}</strong>
                <span>{e.device}</span>
                <span className="timeline__detail">{e.detail}</span>
              </button>
            </li>
          )
        })}
      </ol>
      {visible.length > MAX_ROWS && (
        <p className="muted">Mostrando {MAX_ROWS} de {visible.length} eventos.</p>
      )}

      {current ? (
        <Inspector entry={current} names={names} />
      ) : (
        <p className="muted">Avance ou selecione um evento para inspecionar o pacote.</p>
      )}
    </section>
  )
}

function Layer({ title, rows }: { title: string; rows: [string, string][] }) {
  return (
    <div className="layer">
      <h4>{title}</h4>
      <dl>
        {rows.map(([k, v]) => (
          <div key={k} className="layer__row">
            <dt>{k}</dt>
            <dd>{v}</dd>
          </div>
        ))}
      </dl>
    </div>
  )
}

function frameLayers(f: FrameInfo): ReactElement[] {
  const out: ReactElement[] = [
    <Layer
      key="eth"
      title="Camada 2 · Ethernet"
      rows={[
        ['Origem', f.srcMac],
        ['Destino', f.dstMac],
        ['EtherType', `0x${f.ethertype.toString(16).padStart(4, '0')}`],
        ['Tamanho', `${f.bytes} bytes`],
      ]}
    />,
  ]
  if (f.arp) {
    out.push(
      <Layer
        key="arp"
        title="ARP"
        rows={[
          ['Operação', f.arp.op],
          ['Remetente', `${f.arp.senderIp} (${f.arp.senderMac})`],
          ['Alvo', `${f.arp.targetIp} (${f.arp.targetMac})`],
        ]}
      />,
    )
  }
  if (f.ipv4) {
    out.push(
      <Layer
        key="ip"
        title="Camada 3 · IPv4"
        rows={[
          ['Origem', f.ipv4.src],
          ['Destino', f.ipv4.dst],
          ['TTL', String(f.ipv4.ttl)],
          ['Identificação', String(f.ipv4.id)],
          ['Protocolo', String(f.ipv4.protocol)],
        ]}
      />,
    )
  }
  if (f.icmp) {
    out.push(
      <Layer
        key="icmp"
        title="ICMP"
        rows={[
          ['Tipo', f.icmp.kind],
          ['Código', String(f.icmp.code)],
          ['Identificador', String(f.icmp.id)],
          ['Sequência', String(f.icmp.seq)],
        ]}
      />,
    )
  }
  return out
}

function Inspector({ entry, names }: { entry: LogEntry; names: Record<string, string> }) {
  const link = entry.link
  return (
    <section className="inspector" aria-label="Inspeção do pacote">
      <h3>Inspeção</h3>
      <dl>
        <div className="layer__row">
          <dt>Equipamento</dt>
          <dd>{entry.device}</dd>
        </div>
        <div className="layer__row">
          <dt>Tempo</dt>
          <dd>{(entry.timeUs / 1000).toFixed(2)} ms</dd>
        </div>
        <div className="layer__row">
          <dt>Evento</dt>
          <dd>{KIND_LABEL[entry.kind]}</dd>
        </div>
        {link && (
          <div className="layer__row">
            <dt>Enlace</dt>
            <dd>
              {names[link.fromDevice] ?? link.fromDevice} · {link.fromPort} → {names[link.toDevice] ?? link.toDevice} ·{' '}
              {link.toPort}
            </dd>
          </div>
        )}
      </dl>
      {entry.frame ? frameLayers(entry.frame) : <p className="muted">Este evento não carrega quadro.</p>}
      <p className="muted inspector__detail">{entry.detail}</p>
    </section>
  )
}
