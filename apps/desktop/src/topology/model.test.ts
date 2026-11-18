import { describe, expect, it } from 'vitest'
import {
  addDevice,
  connect,
  emptyTopology,
  moveDevice,
  parseTopology,
  removeDevice,
  removeLink,
  renameDevice,
  serializeTopology,
  type Topology,
} from './model'

function ids() {
  let n = 0
  return () => `id${++n}`
}

function twoPcsAndSwitch(): Topology {
  const f = ids()
  let t = emptyTopology()
  t = addDevice(t, 'pc', { x: 0, y: 0 }, f) // id1, Porta p1
  t = addDevice(t, 'pc', { x: 0, y: 200 }, f) // id2
  t = addDevice(t, 'switch', { x: 300, y: 100 }, f) // id3, p1..p8
  return t
}

describe('addDevice', () => {
  it('creates a device with the expected number of ports and numbered name', () => {
    const t = twoPcsAndSwitch()
    expect(t.devices.map((d) => d.name)).toEqual(['PC 1', 'PC 2', 'Switch 1'])
    expect(t.devices[2].ports).toHaveLength(8)
    expect(t.devices[0].ports).toEqual([{ id: 'p1', name: 'Porta 1' }])
  })

  it('does not mutate the input topology', () => {
    const before = emptyTopology()
    addDevice(before, 'pc', { x: 0, y: 0 }, ids())
    expect(before.devices).toHaveLength(0)
  })
})

describe('renameDevice', () => {
  it('trims and stores the new name', () => {
    const t = twoPcsAndSwitch()
    const r = renameDevice(t, 'id1', '  Estação A  ')
    expect(r.ok && r.value.devices[0].name).toBe('Estação A')
  })

  it('rejects empty and too-long names', () => {
    const t = twoPcsAndSwitch()
    expect(renameDevice(t, 'id1', '   ').ok).toBe(false)
    expect(renameDevice(t, 'id1', 'x'.repeat(33)).ok).toBe(false)
  })

  it('rejects unknown device ids', () => {
    expect(renameDevice(twoPcsAndSwitch(), 'nope', 'A').ok).toBe(false)
  })
})

describe('connect', () => {
  it('links two free ports on different devices', () => {
    const t = twoPcsAndSwitch()
    const r = connect(
      t,
      { deviceId: 'id1', portId: 'p1' },
      { deviceId: 'id3', portId: 'p1' },
      ids(),
    )
    expect(r.ok).toBe(true)
    if (r.ok) expect(r.value.links).toHaveLength(1)
  })

  it('rejects connecting a device to itself', () => {
    const r = connect(
      twoPcsAndSwitch(),
      { deviceId: 'id3', portId: 'p1' },
      { deviceId: 'id3', portId: 'p2' },
      ids(),
    )
    expect(r.ok).toBe(false)
  })

  it('rejects a port that is already used', () => {
    const f = ids()
    let t = twoPcsAndSwitch()
    const first = connect(t, { deviceId: 'id1', portId: 'p1' }, { deviceId: 'id3', portId: 'p1' }, f)
    if (!first.ok) throw new Error('setup failed')
    t = first.value
    const r = connect(t, { deviceId: 'id2', portId: 'p1' }, { deviceId: 'id3', portId: 'p1' }, f)
    expect(r.ok).toBe(false)
  })

  it('rejects a port that does not exist on the device', () => {
    const r = connect(
      twoPcsAndSwitch(),
      { deviceId: 'id1', portId: 'p9' },
      { deviceId: 'id3', portId: 'p1' },
      ids(),
    )
    expect(r.ok).toBe(false)
  })
})

describe('removal', () => {
  it('removing a device also removes its links', () => {
    const f = ids()
    const linked = connect(
      twoPcsAndSwitch(),
      { deviceId: 'id1', portId: 'p1' },
      { deviceId: 'id3', portId: 'p1' },
      f,
    )
    if (!linked.ok) throw new Error('setup failed')
    const t = removeDevice(linked.value, 'id3')
    expect(t.devices.map((d) => d.id)).toEqual(['id1', 'id2'])
    expect(t.links).toEqual([])
  })

  it('removeLink keeps devices', () => {
    const f = ids()
    const linked = connect(
      twoPcsAndSwitch(),
      { deviceId: 'id1', portId: 'p1' },
      { deviceId: 'id3', portId: 'p1' },
      f,
    )
    if (!linked.ok) throw new Error('setup failed')
    const t = removeLink(linked.value, linked.value.links[0].id)
    expect(t.links).toEqual([])
    expect(t.devices).toHaveLength(3)
  })
})

describe('moveDevice', () => {
  it('updates only the target position', () => {
    const t = moveDevice(twoPcsAndSwitch(), 'id2', 555, 666)
    expect(t.devices[1]).toMatchObject({ x: 555, y: 666 })
    expect(t.devices[0]).toMatchObject({ x: 0, y: 0 })
  })
})

describe('JSON round trip and validation', () => {
  it('serializes and parses back to an equal topology', () => {
    const f = ids()
    const linked = connect(
      twoPcsAndSwitch(),
      { deviceId: 'id1', portId: 'p1' },
      { deviceId: 'id3', portId: 'p2' },
      f,
    )
    if (!linked.ok) throw new Error('setup failed')
    const json = serializeTopology(linked.value)
    const parsed = parseTopology(json)
    expect(parsed).toEqual({ ok: true, value: linked.value })
    expect(serializeTopology(parsed.ok ? parsed.value : emptyTopology())).toBe(json)
  })

  it('rejects invalid JSON', () => {
    expect(parseTopology('{not json').ok).toBe(false)
  })

  it('rejects unknown schema versions', () => {
    const json = JSON.stringify({ schemaVersion: 99, devices: [], links: [] })
    const r = parseTopology(json)
    expect(r.ok).toBe(false)
  })

  it('rejects links to missing devices or ports', () => {
    const base = twoPcsAndSwitch()
    const bad = { ...base, links: [{ id: 'l1', from: { deviceId: 'ghost', portId: 'p1' }, to: { deviceId: 'id3', portId: 'p1' } }] }
    expect(parseTopology(JSON.stringify(bad)).ok).toBe(false)
    const badPort = { ...base, links: [{ id: 'l1', from: { deviceId: 'id1', portId: 'p7' }, to: { deviceId: 'id3', portId: 'p1' } }] }
    expect(parseTopology(JSON.stringify(badPort)).ok).toBe(false)
  })

  it('rejects a port used by two links', () => {
    const base = twoPcsAndSwitch()
    const bad = {
      ...base,
      links: [
        { id: 'l1', from: { deviceId: 'id1', portId: 'p1' }, to: { deviceId: 'id3', portId: 'p1' } },
        { id: 'l2', from: { deviceId: 'id2', portId: 'p1' }, to: { deviceId: 'id3', portId: 'p1' } },
      ],
    }
    expect(parseTopology(JSON.stringify(bad)).ok).toBe(false)
  })

  it('rejects duplicate device ids and unknown device kinds', () => {
    const base = twoPcsAndSwitch()
    const dup = { ...base, devices: [base.devices[0], base.devices[0]] }
    expect(parseTopology(JSON.stringify(dup)).ok).toBe(false)
    const kind = { ...base, devices: [{ ...base.devices[0], kind: 'toaster' }] }
    expect(parseTopology(JSON.stringify(kind)).ok).toBe(false)
  })
})
