# Test Table

| Path | Purpose |
|------|---------|
| `layercake-core/` | Rust library crate with core plan/runtime logic, services, database access, exporters, and MCP primitives. |
| `layercake-cli/` | Rust CLI binary for plan execution, generators, migrations, updates, and the interactive console. |
| `layercake-server/` | Rust HTTP/GraphQL server binary for the web UI, collaboration, and MCP endpoints. |
| `frontend/` | Vite + React + Mantine UI with ReactFlow-based plan and graph editors. |
| `src-tauri/` | Tauri 2 shell that embeds the backend, manages SQLite files, and ships the desktop app. |
| `external-modules/` | Optional integrations (e.g. `axum-mcp` transport helpers). |
| `resources/` | Sample projects, Handlebars templates, reference exports, and shared assets. |
| `docs/` | Architecture notes, review logs, and migration plans. |
| `scripts/` | Dev/build helpers (`dev.sh`, platform builds, installers). |

Another paragraph after the table.
