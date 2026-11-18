import { useEffect, useState } from 'react'
import { getEngineStatus, type EngineStatus } from './ipc'

export default function App() {
  const [status, setStatus] = useState<EngineStatus | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    getEngineStatus()
      .then(setStatus)
      .catch((e: unknown) => setError(String(e)))
  }, [])

  return (
    <main>
      <h1>NetForge</h1>
      <p>Fase 1 — fundação. Nenhuma funcionalidade de rede implementada ainda.</p>
      {status && <p>Motor: versão {status.engineVersion}</p>}
      {error && <p role="alert">Falha ao falar com o motor: {error}</p>}
    </main>
  )
}
