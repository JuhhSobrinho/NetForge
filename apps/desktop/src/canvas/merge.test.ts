import { describe, expect, it } from 'vitest'
import type { Edge } from '@xyflow/react'
import type { DeviceFlowNode } from './DeviceNode'
import { applyEdgeSelection, applyNodeSelection, mergeEdges, mergeNodes } from './merge'
import type { Device } from '../topology/model'

function device(id: string, x = 0, y = 0): Device {
  return { id, kind: 'pc', name: id, x, y, ports: [{ id: 'p1', name: 'Porta 1' }] }
}

function node(d: Device, linkedPortIds: string[] = []): DeviceFlowNode {
  return { id: d.id, type: 'device', position: { x: d.x, y: d.y }, selected: false, data: { device: d, linkedPortIds } }
}

function edge(id: string, source: string, target: string): Edge {
  return { id, source, target, sourceHandle: 'p1-out', targetHandle: 'p1-in', selected: false }
}

describe('mergeNodes', () => {
  it('keeps the same object for unchanged devices', () => {
    const a = node(device('a'))
    const b = node(device('b'))
    const prev = [a, b]
    const next = [node(device('a')), node(device('b'))].map((n, i) => ({ ...n, data: { ...n.data, device: prev[i].data.device } }))
    const out = mergeNodes(prev, next)
    expect(out).toBe(prev)
  })

  it('updates only the node whose device changed, and keeps its selection', () => {
    const da = device('a')
    const db = device('b')
    const prev = [{ ...node(da), selected: true }, node(db)]
    const moved = { ...da, x: 50 }
    const next = [node(moved), node(db)]
    const out = mergeNodes(prev, next)
    expect(out[0]).not.toBe(prev[0])
    expect(out[0].position).toEqual({ x: 50, y: 0 })
    expect(out[0].selected).toBe(true)
    expect(out[1]).toBe(prev[1])
  })

  it('updates the node when a linked port changes', () => {
    const d = device('a')
    const prev = [node(d, [])]
    const out = mergeNodes(prev, [node(d, ['p1'])])
    expect(out[0].data.linkedPortIds).toEqual(['p1'])
  })

  it('adds new devices and drops removed ones', () => {
    const prev = [node(device('a')), node(device('b'))]
    const out = mergeNodes(prev, [node(device('a')), node(device('c'))])
    expect(out.map((n) => n.id)).toEqual(['a', 'c'])
  })
})

describe('mergeEdges', () => {
  it('keeps unchanged edges and their objects', () => {
    const prev = [edge('l1', 'a', 'b')]
    expect(mergeEdges(prev, [edge('l1', 'a', 'b')])).toBe(prev)
  })

  it('replaces an edge whose endpoints changed', () => {
    const prev = [edge('l1', 'a', 'b')]
    const out = mergeEdges(prev, [edge('l1', 'a', 'c')])
    expect(out[0].target).toBe('c')
  })
})

describe('selection helpers', () => {
  it('selects only the requested node and copies only changed nodes', () => {
    const prev = [node(device('a')), node(device('b')), node(device('c'))]
    const out = applyNodeSelection(prev, 'b')
    expect(out.map((n) => n.selected)).toEqual([false, true, false])
    expect(out[0]).toBe(prev[0])
    expect(out[2]).toBe(prev[2])
  })

  it('returns the same array when selection does not change', () => {
    const prev = [node(device('a'))]
    expect(applyNodeSelection(prev, null)).toBe(prev)
  })

  it('selects an edge by id', () => {
    const prev = [edge('l1', 'a', 'b'), edge('l2', 'b', 'c')]
    const out = applyEdgeSelection(prev, 'l2')
    expect(out.map((e) => e.selected)).toEqual([false, true])
  })
})
