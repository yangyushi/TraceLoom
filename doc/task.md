# LLM Tracer

## Goal

Create a cross-platform lightweight desktop application to visualise the DAG in LLM agent messaging history.

## Functions

- parse message history files (`.jsonl` files) from different sources (claude/codex/openclaw/...) to build a universal IR
- render the IR as DAG in different themes, including
    - dots: all messages are represented as dots and edges are straight lines.
    - bricks: all messages are represented as rectangles and parallel tool callings and results are represented as 
- allow user to inspect each node/brick and to turn on/off markdown render while inspecting in details.


## Mental Model

Each trajectory can oftem be mapped like a DAG. Below is one example.


```dot
digraph Trajectory {
    rankdir=TB;
    Node [shape=box];

    sp [label="System Prompt"];
    up01 [label="User Prompt"];
    at01 [label="Agent Think"];
    ap01 [label="Agent Response"];

    up02 [label="User Prompt"];
    at02 [label="Agent Think"];
    tc01 [label="Tool Call"];
    tr01 [label="Tool Result"];
    at03 [label="Agent Think"];
    ap02 [label="Agent Response"];

    up03 [label="User Prompt"];
    at04 [label="Agent Think"];
    tc02 [label="Tool Call"];
    tc03 [label="Tool Call"];
    tc04 [label="Tool Call"];
    tr02 [label="Tool Result"];
    tr03 [label="Tool Result"];
    tr04 [label="Tool Result"];
    at05 [label="Agent Think"];
    ap03 [label="Agent Response"];

    sp -> up01 -> at01 -> ap01 -> up02;
    up02 -> at02 -> tc01 -> tr01 -> at03 -> ap02 -> up03;

    up03 -> at04;
    at04 -> tc02 -> tr02 -> at05;
    at04 -> tc03 -> tr03 -> at05;
    at04 -> tc04 -> tr04 -> at05;
    at05 -> ap03;
}

```

## Stack

- Prefer Rust + Tauri, use typescript if have to.

## Examples

sample agent message history files can be found in `samples` folder.
