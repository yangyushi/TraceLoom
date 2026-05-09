import type { TrajectorySource } from "../types/workspace";

export type RenderId = string;

export function namespaceId(sourceId: number, nodeId: string): RenderId {
  return `${sourceId}::${nodeId}`;
}

export function parseRenderId(renderId: RenderId): { sourceId: number; nodeId: string } {
  const parts = renderId.split("::");
  return {
    sourceId: parseInt(parts[0], 10),
    nodeId: parts.slice(1).join("::"),
  };
}

export function findSourceById(sources: TrajectorySource[], id: number): TrajectorySource | undefined {
  return sources.find((s) => s.id === id);
}

export function fileName(path: string): string {
  return path.split(/[/\\]/).pop() ?? path;
}
