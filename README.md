# DevSweep

DevSweep é um utilitário nativo de linha de comando e interface de terminal (TUI) de alto desempenho desenvolvido em Rust, projetado para varredura, análise e limpeza de caches e artefatos de compilação gerados em ambientes de desenvolvimento.

A ferramenta detecta diretórios de compilação descartáveis em múltiplos ecossistemas (Rust, Node.js, Python, Java/Gradle, .NET, C/C++, Flutter, entre outros) e recupera espaço em disco de forma segura, sem colocar o código-fonte em risco.

---

## Recursos

- **Detecção Multiecossistema**: Identifica artefatos de build nos principais ambientes de desenvolvimento:
  - **Rust**: `target/`
  - **Node.js / Web**: `node_modules/`, `.next/`, `.nuxt/`, `.turbo/`, `.dist/`, `.output/`, `.svelte-kit/`, `.astro/`
  - **Python**: `.venv/`, `venv/`, `env/`, `__pycache__/`, `.pytest_cache/`, `.mypy_cache/`, `.ruff_cache/`, `.coverage`
  - **Java / Gradle / Maven**: `build/`, `.gradle/`, `out/`, `target/`
  - **.NET / C#**: `bin/`, `obj/`
  - **C / C++ / CMake**: `build/`, `cmake-build-debug/`, `cmake-build-release/`, `out/`
  - **Flutter / Dart**: `.dart_tool/`, `build/`
  - **Elixir**: `_build/`, `deps/`
  - **PHP / Composer**: `vendor/`
- **Interface de Terminal Interativa (TUI)**: Construída com `ratatui` e `crossterm`, oferecendo varredura em tempo real, ordenação, filtros e inspeção detalhada de projetos.
- **Varredura Multithreaded de Alta Performance**: Utiliza `jwalk` e `rayon` para travessia paralela de diretórios e cálculo rápido do tamanho dos arquivos.
- **Segurança por Padrão**:
  - Validação estrita de caminhos para impedir exclusões acidentais fora da raiz do projeto ou em pastas de sistema.
  - Integração com a Lixeira do sistema operacional por padrão (via APIs do Windows Shell), com opção explícita para exclusão permanente.
- **Modo CLI**: Comandos diretos de varredura e limpeza em lote para automação e scripts.

---

## Instalação e Compilação

### Pré-requisitos
- [Rust Toolchain](https://rustup.rs/) (versão 1.80 ou superior recomendada)
- Ferramentas de compilação C++ (MSVC no Windows, GCC/Clang no Linux/macOS)

### Compilar Binário de Release
```bash
cargo build --release
```

O executável compilado estará disponível em:
- Windows: `target/release/devsweep.exe`
- Linux/macOS: `target/release/devsweep`

---

## Uso

### Modo Interativo (TUI)
Inicie o painel interativo no diretório atual ou em um caminho específico:
```bash
# Varrer diretório atual
devsweep

# Varrer um diretório ou workspace específico
devsweep tui C:\Users\<Usuario>\Documents\GitHub
```

#### Atalhos de Teclado na TUI
| Tecla | Ação |
| :--- | :--- |
| `Up` / `Down` ou `k` / `j` | Mover o cursor de seleção |
| `PgUp` / `PgDown` | Rolar página para cima ou para baixo |
| `Space` | Alternar seleção do projeto destacado |
| `a` | Selecionar / desselecionar todos os projetos |
| `d` / `Delete` | Abrir modal de confirmação de limpeza |
| `s` | Alternar ordenação (Tamanho, Idade, Nome, Ecossistema) e direção |
| `f` | Filtrar por ecossistema |
| `/` | Ativar busca em tempo real |
| `r` | Reiniciar a varredura |
| `?` / `h` | Exibir janela de ajuda e atalhos |
| `Esc` | Limpar busca / Fechar modal ativo |
| `q` | Encerrar o programa |

---

### Interface de Linha de Comando (CLI)

#### Varrer Diretório (Scan)
Executa a varredura não-interativa e exibe um relatório formatado com o espaço recuperável:
```bash
devsweep scan C:\projetos

# Filtrar por ecossistema
devsweep scan C:\projetos --type node

# Exibir apenas projetos inativos há mais de 30 dias
devsweep scan C:\projetos --older-than 30
```

#### Limpar Artefatos (Clean)
Executa a limpeza diretamente pela linha de comando:
```bash
# Limpeza com confirmação interativa
devsweep clean C:\projetos

# Simulação (Dry run - nenhum arquivo é excluído)
devsweep clean C:\projetos --dry-run

# Limpar todos os projetos encontrados sem confirmação interativa
devsweep clean C:\projetos --all --yes

# Limpar apenas ambientes virtuais Python inativos há mais de 60 dias
devsweep clean C:\projetos --type python --older-than 60 --yes

# Exclusão permanente em vez de enviar para a Lixeira
devsweep clean C:\projetos --type rust --permanent --yes
```

---

## Arquitetura de Segurança e Validação

1. **Validação de Escopo de Caminho**: Cada candidato à exclusão é verificado para garantir que está localizado estritamente dentro da raiz do projeto correspondente.
2. **Lista Branca (*Whitelist*)**: Apenas pastas explicitamente categorizadas como alvos de compilação (ex: `target`, `node_modules`, `.venv`) são elegíveis para exclusão.
3. **Bloqueio de Raiz**: Tentativas de apontar para diretórios raiz, unidades de disco ou a própria pasta do projeto são bloqueadas automaticamente.
4. **Integração com a Lixeira**: As deleções padrão invocam APIs do Shell do sistema operacional (`SHFileOperation` / `IFileOperation` no Windows), permitindo restauração caso necessário.

---

## Licença

Este projeto está sob a licença MIT. Consulte o arquivo [LICENSE](LICENSE) para obter mais detalhes.
