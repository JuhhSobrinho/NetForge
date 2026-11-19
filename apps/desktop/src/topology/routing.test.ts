import { describe, expect, it } from 'vitest'
import { addDevice, emptyTopology, parseTopology, type Topology } from './model'

function base(kind: 'pc' | 'router' | 'switch' | 'server'): Topology {
  return addDevice(emptyTopology(), kind, { x: 0, y: 0 }, () => 'd1')
}

describe('parseTopology with routing fields', () => {
  it('accepts a router with a valid static route', () => {
    const t = base('router')
    t.devices[0].routes = [{ network: '10.0.2.0', prefixLen: 24, nextHop: '10.0.9.2' }]
    expect(parseTopology(JSON.stringify(t)).ok).toBe(true)
  })

  it('accepts a host with a default gateway', () => {
    const t = base('pc')
    t.devices[0].gateway = '10.0.1.1'
    expect(parseTopology(JSON.stringify(t)).ok).toBe(true)
  })

  it('rejects a gateway on a switch or router', () => {
    const sw = base('switch')
    sw.devices[0].gateway = '10.0.1.1'
    expect(parseTopology(JSON.stringify(sw)).ok).toBe(false)
  })

  it('rejects static routes on hosts', () => {
    const t = base('pc')
    t.devices[0].routes = [{ network: '10.0.2.0', prefixLen: 24, nextHop: '10.0.9.2' }]
    expect(parseTopology(JSON.stringify(t)).ok).toBe(false)
  })

  it('rejects malformed routes and gateways', () => {
    const r = base('router')
    r.devices[0].routes = [{ network: '999.0.0.0', prefixLen: 24, nextHop: '10.0.9.2' }]
    expect(parseTopology(JSON.stringify(r)).ok).toBe(false)
    r.devices[0].routes = [{ network: '10.0.2.0', prefixLen: 40, nextHop: '10.0.9.2' }]
    expect(parseTopology(JSON.stringify(r)).ok).toBe(false)
    const h = base('pc')
    h.devices[0].gateway = 'não é ip'
    expect(parseTopology(JSON.stringify(h)).ok).toBe(false)
  })

  it('rejects a non-boolean shutdown flag', () => {
    const t = base('pc')
    const json = JSON.parse(JSON.stringify(t))
    json.devices[0].ports[0].shutdown = 'sim'
    expect(parseTopology(JSON.stringify(json)).ok).toBe(false)
  })
})
