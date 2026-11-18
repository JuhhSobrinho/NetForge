import type { Edge } from '@xyflow/react'
import type { DeviceFlowNode } from './DeviceNode'

/**
 * Funções puras que atualizam o estado do React Flow sem recriar objetos que não mudaram.
 * Recriar todos os nós a cada seleção fazia o React Flow refazer medidas e handles
 * (um quadro de ~300 ms com 200 equipamentos; ver docs/TECHNICAL_DECISIONS.md, ADR-003).
 */

export function sameList(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((v, i) => v === b[i])
}

/** Mantém o nó antigo quando o equipamento e as portas ligadas são os mesmos. */
export function mergeNodes(prev: DeviceFlowNode[], next: DeviceFlowNode[]): DeviceFlowNode[] {
  const byId = new Map(prev.map((n) => [n.id, n]))
  const out = next.map((n) => {
    const old = byId.get(n.id)
    if (!old) return n
    if (old.data.device === n.data.device && sameList(old.data.linkedPortIds, n.data.linkedPortIds)) return old
    // Preserva medidas internas e estado de seleção do nó antigo.
    return { ...old, data: n.data, position: n.position }
  })
  const same = out.length === prev.length && out.every((n, i) => n === prev[i])
  return same ? prev : out
}

/** Mantém a aresta antiga quando origem, destino e handles são iguais. */
export function mergeEdges(prev: Edge[], next: Edge[]): Edge[] {
  const byId = new Map(prev.map((e) => [e.id, e]))
  const out = next.map((e) => {
    const old = byId.get(e.id)
    if (
      old &&
      old.source === e.source &&
      old.target === e.target &&
      old.sourceHandle === e.sourceHandle &&
      old.targetHandle === e.targetHandle
    )
      return old
    if (old) return { ...old, ...e, selected: old.selected }
    return e
  })
  const same = out.length === prev.length && out.every((e, i) => e === prev[i])
  return same ? prev : out
}

/** Atualiza só o flag `selected`, copiando apenas os nós que mudam. */
export function applyNodeSelection(prev: DeviceFlowNode[], selectedId: string | null): DeviceFlowNode[] {
  let changed = false
  const out = prev.map((n) => {
    const selected = n.id === selectedId
    if (n.selected === selected) return n
    changed = true
    return { ...n, selected }
  })
  return changed ? out : prev
}

export function applyEdgeSelection(prev: Edge[], linkId: string | null): Edge[] {
  let changed = false
  const out = prev.map((e) => {
    const selected = e.id === linkId
    if (!!e.selected === selected) return e
    changed = true
    return { ...e, selected }
  })
  return changed ? out : prev
}
