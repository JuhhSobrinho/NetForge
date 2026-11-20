import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { Topology } from '../topology/model'

interface LabSummary {
  id: string
  title: string
  objectives: number
}

interface LabSpec {
  id: string
  title: string
  statement: string
  topology: Topology
  objectives: { description: string; sourceId: string; target: string; expectSuccess: boolean }[]
}

interface ObjectiveResult {
  description: string
  passed: boolean
  detail: string
}

/** Laboratórios prontos: carrega a topologia inicial e verifica os objetivos com o motor. */
export function LabsPanel({ topology, onLoad }: { topology: Topology; onLoad: (t: Topology) => void }) {
  const [labs, setLabs] = useState<LabSummary[]>([])
  const [selected, setSelected] = useState('')
  const [lab, setLab] = useState<LabSpec | null>(null)
  const [results, setResults] = useState<ObjectiveResult[] | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    invoke<LabSummary[]>('list_labs')
      .then((l) => {
        setLabs(l)
        if (l.length > 0) setSelected(l[0].id)
      })
      .catch((e: unknown) => setError(String(e)))
  }, [])

  const open = async () => {
    setError(null)
    setResults(null)
    try {
      setLab(await invoke<LabSpec>('get_lab', { id: selected }))
    } catch (e) {
      setError(String(e))
    }
  }

  const load = () => {
    if (lab) onLoad(lab.topology)
    setResults(null)
  }

  const check = async (topology: Topology) => {
    if (!lab) return
    setError(null)
    try {
      setResults(await invoke<ObjectiveResult[]>('check_lab', { topology, labId: lab.id }))
    } catch (e) {
      setError(String(e))
    }
  }

  return (
    <section className="labs" aria-label="Laboratórios">
      <h2>Laboratórios</h2>
      {labs.length === 0 ? (
        <p className="muted">Nenhum laboratório disponível.</p>
      ) : (
        <>
          <label htmlFor="lab-select">Escolha um laboratório</label>
          <select id="lab-select" value={selected} onChange={(e) => setSelected(e.target.value)}>
            {labs.map((l) => (
              <option key={l.id} value={l.id}>
                {l.title}
              </option>
            ))}
          </select>
          <div className="actions">
            <button onClick={open} disabled={!selected}>
              Abrir enunciado
            </button>
          </div>
        </>
      )}

      {error && <p className="status--error ping__result">{error}</p>}

      {lab && (
        <>
          <p className="labs__statement">{lab.statement}</p>
          <div className="actions">
            <button onClick={load}>Carregar na área de trabalho</button>
          </div>
          <p className="muted labs__hint">
            Carregar substitui a topologia atual (desfazer restaura). Configure pelos terminais e depois verifique.
          </p>
          <div className="actions">
            <button onClick={() => check(topology)} title="Verifica o laboratório com a topologia atual da área de trabalho">
              Verificar objetivos
            </button>
          </div>
          {results && (
            <ul className="labs__results" aria-label="Resultado da verificação">
              {results.map((r, i) => (
                <li key={i} className={r.passed ? 'labs__ok' : 'labs__fail'}>
                  <strong>{r.passed ? '✔' : '✘'} {r.description}</strong>
                  <span className="muted">{r.detail}</span>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </section>
  )
}
