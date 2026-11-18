import { describe, expect, it } from 'vitest'
import { createStressTopology } from './stress'
import { parseTopology, serializeTopology } from './model'

describe('createStressTopology', () => {
  it('builds the requested number of devices and links', () => {
    const t = createStressTopology(200, 300)
    expect(t.devices).toHaveLength(200)
    expect(t.links).toHaveLength(300)
  })

  it('produces a topology that passes project validation', () => {
    const t = createStressTopology(200, 300)
    const parsed = parseTopology(serializeTopology(t))
    expect(parsed.ok).toBe(true)
  })
})
