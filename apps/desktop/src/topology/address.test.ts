import { describe, expect, it } from 'vitest'
import { addDevice, emptyTopology, isValidIpv4, parseTopology, setPortAddress, validateAddress } from './model'

function pcTopology() {
  let n = 0
  return addDevice(emptyTopology(), 'pc', { x: 0, y: 0 }, () => `d${++n}`)
}

describe('isValidIpv4', () => {
  it('accepts dotted quads and rejects malformed values', () => {
    expect(isValidIpv4('192.168.1.10')).toBe(true)
    expect(isValidIpv4('0.0.0.0')).toBe(true)
    expect(isValidIpv4('256.1.1.1')).toBe(false)
    expect(isValidIpv4('1.2.3')).toBe(false)
    expect(isValidIpv4('01.2.3.4')).toBe(false)
    expect(isValidIpv4('a.b.c.d')).toBe(false)
  })
})

describe('validateAddress', () => {
  it('requires ip and prefix together', () => {
    expect(validateAddress(undefined, undefined).ok).toBe(true)
    expect(validateAddress('10.0.0.1', undefined).ok).toBe(false)
    expect(validateAddress(undefined, 24).ok).toBe(false)
  })

  it('limits the prefix to 0–32', () => {
    expect(validateAddress('10.0.0.1', 32).ok).toBe(true)
    expect(validateAddress('10.0.0.1', 33).ok).toBe(false)
    expect(validateAddress('10.0.0.1', -1).ok).toBe(false)
  })
})

describe('setPortAddress', () => {
  it('sets and clears the address of one port', () => {
    const t = pcTopology()
    const set = setPortAddress(t, 'd1', 'p1', '192.168.1.10', 24)
    expect(set.ok).toBe(true)
    if (!set.ok) return
    expect(set.value.devices[0].ports[0]).toMatchObject({ ip: '192.168.1.10', prefixLen: 24 })

    const cleared = setPortAddress(set.value, 'd1', 'p1', '', undefined)
    expect(cleared.ok).toBe(true)
    if (cleared.ok) expect('ip' in cleared.value.devices[0].ports[0]).toBe(false)
  })

  it('rejects invalid addresses and unknown ports', () => {
    const t = pcTopology()
    expect(setPortAddress(t, 'd1', 'p1', '999.1.1.1', 24).ok).toBe(false)
    expect(setPortAddress(t, 'd1', 'p9', '10.0.0.1', 24).ok).toBe(false)
    expect(setPortAddress(t, 'nope', 'p1', '10.0.0.1', 24).ok).toBe(false)
  })

  it('does not mutate the input', () => {
    const t = pcTopology()
    setPortAddress(t, 'd1', 'p1', '10.0.0.1', 24)
    expect(t.devices[0].ports[0].ip).toBeUndefined()
  })
})

describe('parseTopology with addresses', () => {
  it('accepts valid addresses and rejects invalid ones', () => {
    const t = setPortAddress(pcTopology(), 'd1', 'p1', '10.0.0.5', 8)
    if (!t.ok) throw new Error('setup')
    expect(parseTopology(JSON.stringify(t.value)).ok).toBe(true)

    const bad = JSON.parse(JSON.stringify(t.value))
    bad.devices[0].ports[0].prefixLen = 40
    expect(parseTopology(JSON.stringify(bad)).ok).toBe(false)
  })
})
