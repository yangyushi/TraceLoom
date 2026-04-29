import type { Block, Message, Trajectory } from "../types/ir";

export interface MessageItem {
  type: "message";
  message: Message;
}

export interface BlockItem {
  type: "block";
  message: Message;
  block: Block;
}

export type OrderedItem = MessageItem | BlockItem;

const topologicalCache = new WeakMap<Message[], Message[]>();
const orderedItemsCache = new WeakMap<Message[], OrderedItem[]>();
const itemLookupCache = new WeakMap<Trajectory, Map<string, OrderedItem>>();

function timestampValue(message: Message): number {
  return message.timestamp ? Date.parse(message.timestamp) || 0 : 0;
}

export function topologicalMessages(messages: Message[]): Message[] {
  const cached = topologicalCache.get(messages);
  if (cached) return cached;

  const msgMap = new Map(messages.map((message) => [message.id, message]));
  const sourceIndex = new Map(messages.map((message, index) => [message.id, index]));
  const inDegree = new Map<string, number>();
  const adj = new Map<string, string[]>();

  for (const message of messages) {
    inDegree.set(message.id, 0);
  }

  for (const message of messages) {
    if (message.parent_id && msgMap.has(message.parent_id)) {
      adj.set(message.parent_id, [...(adj.get(message.parent_id) ?? []), message.id]);
      inDegree.set(message.id, (inDegree.get(message.id) ?? 0) + 1);
    }
  }

  const result: Message[] = [];
  const visited = new Set<string>();
  const queue = messages.filter((message) => (inDegree.get(message.id) ?? 0) === 0);

  const sortQueue = () => {
    queue.sort((a, b) => {
      const timeDiff = timestampValue(a) - timestampValue(b);
      if (timeDiff !== 0) return timeDiff;
      return (sourceIndex.get(a.id) ?? Number.MAX_SAFE_INTEGER)
        - (sourceIndex.get(b.id) ?? Number.MAX_SAFE_INTEGER);
    });
  };

  while (queue.length > 0) {
    sortQueue();
    const message = queue.shift()!;
    if (visited.has(message.id)) continue;
    visited.add(message.id);
    result.push(message);

    for (const childId of adj.get(message.id) ?? []) {
      const child = msgMap.get(childId);
      if (!child) continue;
      const degree = (inDegree.get(childId) ?? 0) - 1;
      inDegree.set(childId, degree);
      if (degree === 0 && !visited.has(childId)) {
        queue.push(child);
      }
    }
  }

  for (const message of messages) {
    if (!visited.has(message.id)) {
      result.push(message);
    }
  }

  topologicalCache.set(messages, result);
  return result;
}

export function orderedItems(messages: Message[]): OrderedItem[] {
  const cached = orderedItemsCache.get(messages);
  if (cached) return cached;

  const items = topologicalMessages(messages).flatMap((message) => [
    { type: "message" as const, message },
    ...message.blocks.map((block) => ({ type: "block" as const, message, block })),
  ]);
  orderedItemsCache.set(messages, items);
  return items;
}

export function findOrderedItem(trajectory: Trajectory, id: string | null): OrderedItem | null {
  if (!id) return null;
  let lookup = itemLookupCache.get(trajectory);
  if (!lookup) {
    lookup = new Map();
    for (const item of orderedItems(trajectory.messages)) {
      lookup.set(item.type === "message" ? item.message.id : item.block.id, item);
    }
    itemLookupCache.set(trajectory, lookup);
  }
  return lookup.get(id) ?? null;
}
