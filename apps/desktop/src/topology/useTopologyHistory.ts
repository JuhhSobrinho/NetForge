import { useCallback, useState } from 'react'
import { canRedo, canUndo, commit, createHistory, redo, undo, type History } from './history'
import type { Topology } from './model'

/** Estado da topologia com undo/redo. Toda mudança passa por `apply`. */
export function useTopologyHistory(initial: Topology) {
  const [history, setHistory] = useState<History>(() => createHistory(initial))

  const apply = useCallback((change: (t: Topology) => Topology) => {
    setHistory((h) => commit(h, change(h.present)))
  }, [])

  const doUndo = useCallback(() => setHistory((h) => undo(h)), [])
  const doRedo = useCallback(() => setHistory((h) => redo(h)), [])

  return {
    topology: history.present,
    apply,
    undo: doUndo,
    redo: doRedo,
    canUndo: canUndo(history),
    canRedo: canRedo(history),
  }
}
