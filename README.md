# TraceLoom

TraceLoom is a cross-platform desktop application for visualising LLM agent message histories as DAGs.

## What It Does

TraceLoom reads `.jsonl` log files from LLM agents and visualise, supported softwares include,

- Claude
- Codex
- OpenClaw

## Build from source

```bash
npm install

# Development
make dev

# Release build
make
```

Windows users can run the equivalent PowerShell commands:

```powershell
.\build.ps1 -Dev
.\build.ps1
```

See [doc/building.md](doc/building.md) for platform prerequisites and troubleshooting.
