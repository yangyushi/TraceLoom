import type { Block } from "../types/ir";

function truncate(text: string, maxLength = 120): string {
  return text.length > maxLength ? `${text.slice(0, maxLength)}...` : text;
}

export function getBlockRawText(block: Block): string {
  const content = block.content;
  switch (content.type) {
    case "Text":
      return content.data;
    case "Thinking":
      return content.data.text;
    case "ToolUse":
      return `${content.data.name}\n${JSON.stringify(content.data.input, null, 2)}`;
    case "ToolResult":
      return content.data.output;
    case "Snapshot":
      return content.data.description;
    case "Custom":
      return JSON.stringify(content.data.payload, null, 2);
    default:
      return "";
  }
}

export function getBlockPreview(block: Block): string {
  const content = block.content;
  switch (content.type) {
    case "Text":
      return truncate(content.data);
    case "ToolUse":
      return `Tool: ${content.data.name}`;
    case "ToolResult":
      return `Result: ${truncate(content.data.output)}`;
    case "Thinking":
      return content.data.encrypted ? "[encrypted reasoning]" : truncate(content.data.text);
    case "Snapshot":
      return content.data.description;
    case "Custom":
      return `[${content.data.kind}]`;
    default:
      return `[${(content as { type: string }).type}]`;
  }
}
