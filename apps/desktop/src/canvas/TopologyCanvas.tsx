import { useEffect, useCallback } from 'react'
import {
  Background,
  BackgroundVariant,
  ConnectionMode,
  Controls,
  MiniMap,
  ReactFlow,
  useEdgesState,
  useNodesState,
  type Connection,
  type Edge,
} from '@xyflow/react'
import '@xyflow/react/dist/style.css'
import { DeviceNode, type DeviceFlowNode } from './DeviceNode'
import { handleToPortId } from './handles'
import { applyEdgeSelection, applyNodeSelection, mergeEdges, mergeNodes } from './merge'
import type { PortRef, Topology } from '../topology/model'

export type Selection = { deviceId: string | null; linkId: string | null }

interface Props {
  topology: Topology
  selection: Selection
  onSelectionChange: (selection: Selection) => void
  onMoveDevices: (moves: { id: string; x: number; y: number }[]) => void
  onConnectPorts: (a: PortRef, b: PortRef) => void
  onDelete: (deviceIds: string[], linkIds: string[]) => void
}

const nodeTypes = { device: DeviceNode }

function toNodes(t: Topology): DeviceFlowNode[] {
  const linkedByDevice = new Map<string, string[]>()
  for (const l of t.links) {
    for (const ref of [l.from, l.to]) {
      const list = linkedByDevice.get(ref.deviceId) ?? []
      list.push(ref.portId)
      linkedByDevice.set(ref.deviceId, list)
    }
  }
  return t.devices.map((device) => ({
    id: device.id,
    type: 'device',
    position: { x: device.x, y: device.y },
    selected: false,
    data: { device, linkedPortIds: linkedByDevice.get(device.id) ?? [] },
  }))
}

function toEdges(t: Topology): Edge[] {
  return t.links.map((l) => ({
    id: l.id,
    source: l.from.deviceId,
    sourceHandle: `${l.from.portId}-out`,
    target: l.to.deviceId,
    targetHandle: `${l.to.portId}-in`,
    selected: false,
  }))
}

export function TopologyCanvas({
  topology,
  selection,
  onSelectionChange,
  onMoveDevices,
  onConnectPorts,
  onDelete: onDeleteElements,
}: Props) {
  const [nodes, setNodes, onNodesChange] = useNodesState<DeviceFlowNode>(toNodes(topology))
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>(toEdges(topology))

  // Modelo mudou (edição, undo/redo, abrir arquivo): atualiza só o que mudou.
  // Os objetos antigos são preservados para não perder medidas internas do React Flow.
  useEffect(() => {
    setNodes((prev) => mergeNodes(prev, toNodes(topology)))
    setEdges((prev) => mergeEdges(prev, toEdges(topology)))
  }, [topology, setNodes, setEdges])

  // Seleção mudou: só os flags `selected` são atualizados.
  useEffect(() => {
    setNodes((prev) => applyNodeSelection(prev, selection.deviceId))
  }, [selection.deviceId, setNodes])
  useEffect(() => {
    setEdges((prev) => applyEdgeSelection(prev, selection.linkId))
  }, [selection.linkId, setEdges])

  const onConnect = useCallback(
    (c: Connection) => {
      const a = handleToPortId(c.sourceHandle)
      const b = handleToPortId(c.targetHandle)
      if (!c.source || !c.target || !a || !b) return
      onConnectPorts(
        { deviceId: c.source, portId: a },
        { deviceId: c.target, portId: b },
      )
    },
    [onConnectPorts],
  )

  const onNodeDragStop = useCallback(
    (_: unknown, node: DeviceFlowNode, dragged: DeviceFlowNode[]) => {
      const moved = dragged.length > 0 ? dragged : [node]
      onMoveDevices(moved.map((n) => ({ id: n.id, x: n.position.x, y: n.position.y })))
    },
    [onMoveDevices],
  )

  const onSelection = useCallback(
    ({ nodes: selNodes, edges: selEdges }: { nodes: DeviceFlowNode[]; edges: Edge[] }) => {
      onSelectionChange({
        deviceId: selNodes.length === 1 ? selNodes[0].id : null,
        linkId: selNodes.length === 0 && selEdges.length === 1 ? selEdges[0].id : null,
      })
    },
    [onSelectionChange],
  )

  const onDelete = useCallback(
    ({ nodes: del, edges: delEdges }: { nodes: DeviceFlowNode[]; edges: Edge[] }) => {
      onDeleteElements(
        del.map((n) => n.id),
        delEdges.map((e) => e.id),
      )
    },
    [onDeleteElements],
  )

  return (
    <div className="canvas">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onNodeDragStop={onNodeDragStop}
        onSelectionChange={onSelection}
        onDelete={onDelete}
        deleteKeyCode={['Delete']}
        connectionMode={ConnectionMode.Loose}
        fitView
        minZoom={0.1}
        maxZoom={2}
        proOptions={{ hideAttribution: false }}
      >
        <Background variant={BackgroundVariant.Dots} gap={20} size={1} />
        <Controls />
        <MiniMap pannable zoomable />
      </ReactFlow>
    </div>
  )
}
