# DevSweep

DevSweep is a high-performance, native command-line and terminal user interface (TUI) utility built in Rust for scanning, analyzing, and cleaning developer build caches and artifacts.

It detects disposable build targets across various ecosystems (Rust, Node.js, Python, Java/Gradle, .NET, C/C++, Flutter, and more) and safely reclaims disk space without risking source code.

---

## Features

- **Multi-Ecosystem Detection**: Identifies build artifacts across major programming ecosystems:
  - **Rust**: `target/`
  - **Node.js / Web**: `node_modules/`, `.next/`, `.nuxt/`, `.turbo/`, `.dist/`, `.output/`, `.svelte-kit/`, `.astro/`
  - **Python**: `.venv/`, `venv/`, `env/`, `__pycache__/`, `.pytest_cache/`, `.mypy_cache/`, `.ruff_cache/`, `.coverage`
  - **Java / Gradle / Maven**: `build/`, `.gradle/`, `out/`, `target/`
  - **.NET / C#**: `bin/`, `obj/`
  - **C / C++ / CMake**: `build/`, `cmake-build-debug/`, `cmake-build-release/`, `out/`
  - **Flutter / Dart**: `.dart_tool/`, `build/`
  - **Elixir**: `_build/`, `deps/`
  - **PHP / Composer**: `vendor/`
- **Interactive Terminal UI (TUI)**: Built with `ratatui` and `crossterm` providing live scanning, sorting, filtering, and instant project inspection.
- **Fast Multithreaded Scanning**: Uses `jwalk` and `rayon` for parallel directory traversal and size computation.
- **Safe by Design**: 
  - Validates artifact paths to prevent accidental deletion of source directories or system paths.
  - Defaults to the system Recycle Bin (via OS Shell API) with an explicit opt-in for permanent deletion.
- **CLI Mode**: Automated scanning and batch cleaning commands for scripts and CI pipelines.

---

## Installation and Building

### Prerequisites
- [Rust Toolchain](https://rustup.rs/) (version 1.80+ recommended)
- C++ Build Tools (MSVC on Windows, GCC/Clang on Linux/macOS)

### Build Release Binary
```bash
cargo build --release
```
The compiled executable will be located at:
- Windows: `target/release/devsweep.exe`
- Linux/macOS: `target/release/devsweep`

---

## Usage

### Interactive TUI Mode
Launch the interactive dashboard in the current directory or target path:
```bash
# Scan current directory
devsweep

# Scan a specific directory or workspace
devsweep tui C:\Users\<Username>\Documents\GitHub
```

#### TUI Keyboard Navigation
| Key | Action |
| :--- | :--- |
| `Up` / `Down` or `k` / `j` | Move selection cursor up and down |
| `PgUp` / `PgDown` | Scroll page up or down |
| `Space` | Toggle selection for highlighted project |
| `a` | Toggle select all / deselect all |
| `d` / `Delete` | Open cleanup confirmation modal |
| `s` | Cycle sorting field (Size, Age, Name, Ecosystem) and direction |
| `f` | Filter by ecosystem |
| `/` | Live search filter |
| `r` | Restart directory scan |
| `?` / `h` | Display help popup |
| `Esc` | Clear search filter / Dismiss modal |
| `q` | Quit application |

---

### Command Line Interface (CLI)

#### Scan Directory
Scan a directory non-interactively and print a structured summary of reclaimable storage:
```bash
devsweep scan C:\projects

# Filter by ecosystem
devsweep scan C:\projects --type node

# Show projects inactive for more than 30 days
devsweep scan C:\projects --older-than 30
```

#### Clean Artifacts
Clean build caches directly from the command line:
```bash
# Clean with interactive prompt
devsweep clean C:\projects

# Dry run simulation (no files modified)
devsweep clean C:\projects --dry-run

# Clean all discovered projects without confirmation prompt
devsweep clean C:\projects --all --yes

# Clean only Python virtual environments inactive for over 60 days
devsweep clean C:\projects --type python --older-than 60 --yes

# Permanently delete instead of sending to the Recycle Bin
devsweep clean C:\projects --type rust --permanent --yes
```

---

## Safety and Validation Architecture

1. **Path Boundary Validation**: Every deletion candidate is checked to ensure it is contained strictly within a recognized project root.
2. **Whitelist Verification**: Only explicitly categorized build directory names (e.g., `target`, `node_modules`) are eligible for deletion.
3. **Root Guard**: Attempts to target root directories, drives, or top-level project folders are blocked automatically.
4. **Recycle Bin Integration**: Deletions invoke platform shell APIs (`IFileOperation` / `SHFileOperation` on Windows) to allow file restoration if needed.

---

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
