/** Altura do cabeçalho e de cada linha de porta. Os handles são posicionados com base nelas. */
export const HEADER_HEIGHT = 40
export const PORT_ROW_HEIGHT = 26

/**
 * Cada porta tem dois handles: entrada à esquerda (`<porta>-in`) e saída à direita (`<porta>-out`).
 * Ligações usam o id da porta sem o sufixo (ver `handleToPortId`).
 */
export function handleToPortId(handle: string | null | undefined): string | null {
  if (!handle) return null
  return handle.replace(/-(in|out)$/, '')
}
