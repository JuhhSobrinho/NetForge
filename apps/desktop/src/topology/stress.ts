// Topologia sintética para medir o canvas (ADR-003). Usada somente com ?stress=N em modo DEV.
// As portas são usadas uma única vez, então o resultado também passa em parseTopology.
import { DEVICE_LABEL, emptyTopology, type Device, type Link, type Topology } from './model'

export function createStressTopology(deviceCount: number, linkCount: number): Topology {
  const columns = 20
  const devices: Device[] = Array.from({ length: deviceCount }, (_, i) => ({
    id: `d${i}`,
    kind: 'router',
    name: `${DEVICE_LABEL.router} ${i + 1}`,
    x: (i % columns) * 220,
    y: Math.floor(i / columns) * 160,
    ports: [1, 2, 3, 4].map((n) => ({ id: `p${n}`, name: `Porta ${n}` })),
  }))

  const links: Link[] = []
  // Primeiro laço: d[i].p1 -> d[i+1].p2. Segundo: d[i].p3 -> d[i+10].p4.
  for (let i = 0; i + 1 < deviceCount && links.length < linkCount; i++) {
    links.push({
      id: `l${links.length}`,
      from: { deviceId: `d${i}`, portId: 'p1' },
      to: { deviceId: `d${i + 1}`, portId: 'p2' },
    })
  }
  for (let i = 0; i + 10 < deviceCount && links.length < linkCount; i++) {
    links.push({
      id: `l${links.length}`,
      from: { deviceId: `d${i}`, portId: 'p3' },
      to: { deviceId: `d${i + 10}`, portId: 'p4' },
    })
  }

  return { ...emptyTopology(), devices, links }
}
