// Histórico de undo/redo (FEATURES.md §1). Guarda snapshots imutáveis da topologia.
import type { Topology } from './model'

export const HISTORY_LIMIT = 100

export interface History {
  present: Topology
  past: Topology[]
  future: Topology[]
}

export function createHistory(initial: Topology): History {
  return { present: initial, past: [], future: [] }
}

/** Registra uma nova versão. Não faz nada se a topologia não mudou. */
export function commit(h: History, next: Topology): History {
  if (next === h.present) return h
  const past = [...h.past, h.present]
  return {
    present: next,
    past: past.length > HISTORY_LIMIT ? past.slice(past.length - HISTORY_LIMIT) : past,
    future: [],
  }
}

export function undo(h: History): History {
  if (h.past.length === 0) return h
  const previous = h.past[h.past.length - 1]
  return {
    present: previous,
    past: h.past.slice(0, -1),
    future: [h.present, ...h.future],
  }
}

export function redo(h: History): History {
  if (h.future.length === 0) return h
  const [next, ...rest] = h.future
  return {
    present: next,
    past: [...h.past, h.present],
    future: rest,
  }
}

export const canUndo = (h: History) => h.past.length > 0
export const canRedo = (h: History) => h.future.length > 0
