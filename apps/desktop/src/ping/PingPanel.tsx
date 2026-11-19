import { useMemo, useState } from 'react'
import { pickCapturePath } from '../project/fileActions'
import type { Topology } from '../topology/model'
import { exportPcap, runPing, type LogKind, type PingOutcome } from './engine'

const MAX_LOG_LINES = 500
const KIND_LABEL: Record<LogKind, string> = {
  tx: 'TX',
  rx: 'RX',
  learn: 'aprende',
  flood: 'inunda',
  forward: 'encaminha',
  drop: 'descarta',
  info: 'info',
}

export function PingPanel({ topology }: { topology: Topology }) {
  const hosts = useMemo(
    () => topology.devices.filter((d) => (d.kind === 'pc' || d.kind === 'server') && d.ports[0]?.ip !== undefined),
    [topology],
  )
  const [sourceId, setSourceId] = useState<string>('')
  const [target, setTarget] = useState('')
  const [count, setCount] = useState(4)
  const [busy, setBusy] = useState(false)
  const [outcome, setOutcome] = useState<PingOutcome | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)

  // Se a origem escolhida deixou de ser um host com endereço, usa o primeiro disponível.
  const source = hosts.some((h) => h.id === sourceId) ? sourceId : (hosts[0]?.id ?? '')

  const run = async () => {
    setBusy(true)
    setError(null)
    setNotice(null)
    try {
      const result = await runPing(topology, source, target, count)
      setOutcome(result)
      setError(result.error)
    } catch (e) {
      setOutcome(null)
      setError(String(e))
    } finally {
      setBusy(false)
    }
  }

  const exportCapture = async () => {
    if (!outcome) return
    try {
      const path = await pickCapturePath('captura-ping.pcap')
      if (!path) return
      await exportPcap(path, outcome.pcap)
      setNotice(`Captura salva em ${path}`)
    } catch (e) {
      setError(`Não foi possível exportar: ${String(e)}`)
    }
  }

  const lines = outcome?.log.slice(0, MAX_LOG_LINES) ?? []

  return (
    <section className="ping" aria-label="Teste de ping">
      <h2>Teste de ping</h2>
      {hosts.length === 0 ? (
        <p className="muted">Defina um endereço IP em um PC ou servidor ligado a outro equipamento.</p>
      ) : (
        <>
          <label htmlFor="ping-source">Origem</label>
          <select id="ping-source" value={source} onChange={(e) => setSourceId(e.target.value)}>
            {hosts.map((h) => (
              <option key={h.id} value={h.id}>
                {h.name} ({h.ports[0].ip})
              </option>
            ))}
          </select>

          <label htmlFor="ping-target">Destino (IPv4)</label>
          <input
            id="ping-target"
            value={target}
            placeholder="ex.: 10.0.0.2"
            onChange={(e) => setTarget(e.target.value)}
          />

          <label htmlFor="ping-count">Pacotes (1–10)</label>
          <input
            id="ping-count"
            type="number"
            min={1}
            max={10}
            value={count}
            onChange={(e) => setCount(Number(e.target.value))}
          />

          <div className="actions">
            <button onClick={run} disabled={busy || !source || target.trim() === ''}>
              {busy ? 'Executando…' : 'Executar ping'}
            </button>
            <button onClick={exportCapture} disabled={!outcome}>
              Exportar PCAP
            </button>
          </div>
        </>
      )}

      {error && <p className="status--error ping__result">{error}</p>}
      {notice && <p className="status--ok ping__result">{notice}</p>}

      {outcome && (
        <>
          <p className="ping__result">
            {outcome.replies.length} de {outcome.transmitted} respostas
            {outcome.replies.length > 0 &&
              ` · RTT ${outcome.replies.map((r) => (r.rttUs / 1000).toFixed(2)).join(' / ')} ms`}
          </p>
          <h3 className="ping__subtitle">Eventos</h3>
          <ol className="ping__log">
            {lines.map((e, i) => (
              <li key={i}>
                <div className="ping__meta">
                  <span className="muted">{(e.timeUs / 1000).toFixed(2)} ms</span>
                  <strong>{KIND_LABEL[e.kind]}</strong>
                  <span>{e.device}</span>
                </div>
                <div className="ping__detail">{e.detail}</div>
              </li>
            ))}
          </ol>
          {(outcome.log.length > MAX_LOG_LINES || outcome.logTruncated) && (
            <p className="muted">Mostrando {lines.length} de {outcome.log.length} eventos.</p>
          )}
        </>
      )}
    </section>
  )
}
