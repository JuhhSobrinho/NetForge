import { useMemo, useState } from 'react'
import { pickCapturePath } from '../project/fileActions'
import type { DeviceTables } from '../cli/engine'
import type { Topology } from '../topology/model'
import { Timeline } from '../sim/Timeline'
import { exportPcap, runPing, type ActiveLink, type PingOutcome } from './engine'

export function PingPanel({
  topology,
  onRuntime,
  onActiveLink,
}: {
  topology: Topology
  onRuntime: (t: DeviceTables[]) => void
  onActiveLink: (link: ActiveLink | null) => void
}) {
  const hosts = useMemo(
    () => topology.devices.filter((d) => (d.kind === 'pc' || d.kind === 'server') && d.ports[0]?.ip !== undefined),
    [topology],
  )
  const names = useMemo(() => Object.fromEntries(topology.devices.map((d) => [d.id, d.name])), [topology])
  const [sourceId, setSourceId] = useState<string>('')
  const [target, setTarget] = useState('')
  const [count, setCount] = useState(4)
  const [busy, setBusy] = useState(false)
  const [outcome, setOutcome] = useState<PingOutcome | null>(null)
  const [runId, setRunId] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)

  // Se a origem escolhida deixou de ser um host com endereço, usa o primeiro disponível.
  const source = hosts.some((h) => h.id === sourceId) ? sourceId : (hosts[0]?.id ?? '')

  const run = async () => {
    setBusy(true)
    setError(null)
    setNotice(null)
    onActiveLink(null)
    try {
      const result = await runPing(topology, source, target, count)
      setOutcome(result)
      setRunId((n) => n + 1)
      onRuntime(result.tables)
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
          <ul className="diagnostics" aria-label="Diagnóstico">
            {outcome.diagnostics.map((f, i) => (
              <li key={i} className={`diagnostics__item diagnostics__item--${f.severity}`}>
                <strong>{f.title}</strong>
                {f.suggestion && <span className="diagnostics__hint">{f.suggestion}</span>}
              </li>
            ))}
          </ul>
          <Timeline
            key={runId}
            entries={outcome.log}
            names={names}
            onActiveLink={onActiveLink}
          />
          {outcome.logTruncated && <p className="muted">O log foi truncado no limite de eventos.</p>}
        </>
      )}
    </section>
  )
}
