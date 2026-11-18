import { Handle, Position, type Node, type NodeProps } from '@xyflow/react'
import { DEVICE_LABEL, type Device } from '../topology/model'
import { HEADER_HEIGHT, PORT_ROW_HEIGHT } from './handles'

export type DeviceNodeData = {
  device: Device
  linkedPortIds: string[]
}
export type DeviceFlowNode = Node<DeviceNodeData, 'device'>

// Cada porta tem dois handles: `<porta>-in` (esquerda) e `<porta>-out` (direita). Ver handles.ts.
export function DeviceNode({ data, selected }: NodeProps<DeviceFlowNode>) {
  const { device, linkedPortIds } = data
  return (
    <div className={`device-node${selected ? ' device-node--selected' : ''}`}>
      <div className="device-node__header">
        <span className="device-node__kind">{DEVICE_LABEL[device.kind]}</span>
        <span className="device-node__name" title={device.name}>
          {device.name}
        </span>
      </div>
      {device.ports.map((port, i) => {
        const top = HEADER_HEIGHT + i * PORT_ROW_HEIGHT + PORT_ROW_HEIGHT / 2
        const linked = linkedPortIds.includes(port.id)
        return (
          <div
            key={port.id}
            className={`device-node__port${linked ? ' device-node__port--linked' : ''}`}
          >
            <Handle type="target" position={Position.Left} id={`${port.id}-in`} style={{ top }} />
            <span>{port.name}</span>
            <Handle type="source" position={Position.Right} id={`${port.id}-out`} style={{ top }} />
          </div>
        )
      })}
    </div>
  )
}
