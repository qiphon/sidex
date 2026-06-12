# AGENTS.md — SideX

## Key Commands

```bash
# Install and setup
npm install
npm run setup                    # Generate extension metadata (REQUIRED before dev/build)

# Development
npm run dev                     # Start Vite dev server
npm run tauri dev               # Full Tauri dev (includes Rust backend)

# Build
npm run build                   # Build frontend
npx tauri build                 # Build Tauri app

# Testing
npm run test                    # TypeScript tests (mocha)
npm run test:git                # Run specific git tests
npm run rust:test               # Rust tests

# Code quality
npm run lint && npm run lint:fix
npm run format && npm run format:check
npm run rust:check && npm run rust:clippy && npm run rust:fmt
```

## Important Constraints

- **Always run `npm run setup` before `npm run dev` or `npm run build`** — generates extension metadata from `extensions/` directory
- Build requires high memory: `NODE_OPTIONS="--max-old-space-size=12288"`
- First Tauri build takes 5-10 minutes (Rust compilation)

## Project Structure

| Directory | Purpose |
|-----------|---------|
| `src/` | TypeScript frontend entry point |
| `src/vs/` | VSCode workbench移植 (TypeScript) |
| `src-tauri/src/` | Rust后端 |
| `src-tauri/src/commands/` | Rust命令模块 |
| `crates/` | Rust workspace crates (terminal, git, workspace, extensions等) |

## TypeScript Conventions

- 使用 `.js` 扩展名进行导入 (ES modules)
- 使用 VSCode的DI模式: `@inject` 装饰器
- 遵循现有VSCode代码模式

## Rust Conventions

- 命令放在 `src-tauri/src/commands/`
- 在 `src-tauri/src/lib.rs` 注册新命令
- 返回类型使用 `Result<T, String>`
- 使用 `tokio` 进行异步处理

## Reference

- `agent.md` — 详细项目架构说明
- `ARCHITECTURE.md` — VSCode到Tauri的架构映射
- `CONTRIBUTING.md` — 贡献指南
