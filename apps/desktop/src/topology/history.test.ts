import { describe, expect, it } from 'vitest'
import { canRedo, canUndo, commit, createHistory, HISTORY_LIMIT, redo, undo } from './history'
import { addDevice, emptyTopology } from './model'

const idA = () => 'a'
const idB = () => 'b'

describe('history', () => {
  it('undo and redo move through committed states', () => {
    let h = createHistory(emptyTopology())
    h = commit(h, addDevice(h.present, 'pc', { x: 0, y: 0 }, idA))
    h = commit(h, addDevice(h.present, 'pc', { x: 1, y: 1 }, idB))
    expect(h.present.devices).toHaveLength(2)

    h = undo(h)
    expect(h.present.devices).toHaveLength(1)
    expect(canRedo(h)).toBe(true)

    h = redo(h)
    expect(h.present.devices).toHaveLength(2)
    expect(canRedo(h)).toBe(false)
  })

  it('a new commit after undo clears the redo branch', () => {
    let h = createHistory(emptyTopology())
    h = commit(h, addDevice(h.present, 'pc', { x: 0, y: 0 }, idA))
    h = undo(h)
    h = commit(h, addDevice(h.present, 'switch', { x: 0, y: 0 }, idB))
    expect(canRedo(h)).toBe(false)
    expect(h.present.devices[0].kind).toBe('switch')
  })

  it('does nothing when there is nothing to undo or redo', () => {
    const h = createHistory(emptyTopology())
    expect(undo(h)).toBe(h)
    expect(redo(h)).toBe(h)
    expect(canUndo(h)).toBe(false)
  })

  it('ignores a commit of the same object', () => {
    const h = createHistory(emptyTopology())
    expect(commit(h, h.present)).toBe(h)
  })

  it('limits the number of undo steps', () => {
    let h = createHistory(emptyTopology())
    for (let i = 0; i < HISTORY_LIMIT + 20; i++) {
      h = commit(h, addDevice(h.present, 'pc', { x: i, y: 0 }, () => `d${i}`))
    }
    expect(h.past).toHaveLength(HISTORY_LIMIT)
  })
})
