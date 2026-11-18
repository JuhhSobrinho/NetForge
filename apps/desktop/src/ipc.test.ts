import { describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'engine_status') return { engineVersion: '0.1.0' }
    throw new Error(`unexpected command ${cmd}`)
  }),
}))

import { getEngineStatus } from './ipc'

describe('getEngineStatus', () => {
  it('calls the engine_status command and returns its typed payload', async () => {
    const status = await getEngineStatus()
    expect(status).toEqual({ engineVersion: '0.1.0' })
  })
})
