import { useEffect, useRef, useState } from 'react'
import type { Device, Topology } from '../topology/model'
import { cliExecute, type CliSession } from './engine'

const START: CliSession = { mode: 'user', interface: null }

/** Terminal de um equipamento. A sessão (modo) fica aqui; a configuração vai para o modelo. */
export function DeviceTerminal({
  topology,
  device,
  onTopology,
}: {
  topology: Topology
  device: Device
  onTopology: (t: Topology) => void
}) {
  const [session, setSession] = useState<CliSession>(START)
  const [prompt, setPrompt] = useState('')
  const [lines, setLines] = useState<string[]>([])
  const [input, setInput] = useState('')
  const [busy, setBusy] = useState(false)
  const outputRef = useRef<HTMLDivElement>(null)

  // Obtém o prompt inicial do equipamento.
  useEffect(() => {
    cliExecute(topology, device.id, START, '')
      .then((r) => setPrompt(r.prompt))
      .catch((e: unknown) => setLines([`% ${String(e)}`]))
    // Só na montagem: o terminal é recriado ao trocar de equipamento (key no pai).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => {
    outputRef.current?.scrollTo({ top: outputRef.current.scrollHeight })
  }, [lines])

  const submit = async () => {
    const line = input.trim()
    setInput('')
    if (busy) return
    setLines((l) => [...l, `${prompt} ${line}`])
    if (line === '') return
    setBusy(true)
    try {
      const r = await cliExecute(topology, device.id, session, line)
      setLines((l) => [...l, ...r.output])
      setSession(r.session)
      setPrompt(r.prompt)
      if (r.topology) {
        onTopology(r.topology)
        setLines((l) => [...l, '(configuração aplicada)'])
      }
    } catch (e) {
      setLines((l) => [...l, `% ${String(e)}`])
    } finally {
      setBusy(false)
    }
  }

  return (
    <section className="terminal" aria-label={`Terminal de ${device.name}`}>
      <h3 className="terminal__title">Terminal</h3>
      <div className="terminal__output" ref={outputRef} role="log">
        {lines.map((l, i) => (
          <div key={i} className={l.startsWith('%') ? 'terminal__error' : undefined}>
            {l || ' '}
          </div>
        ))}
      </div>
      <form
        className="terminal__input"
        onSubmit={(e) => {
          e.preventDefault()
          void submit()
        }}
      >
        <span className="terminal__prompt">{prompt}</span>
        <input
          aria-label="Comando"
          value={input}
          disabled={busy}
          autoComplete="off"
          spellCheck={false}
          onChange={(e) => setInput(e.target.value)}
        />
      </form>
      <p className="muted terminal__hint">Ex.: enable, conf t, interface p1, ip address 10.0.0.1 255.255.255.0</p>
    </section>
  )
}
