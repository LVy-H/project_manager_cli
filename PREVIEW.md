# Sentinel: Your Second Brain for Development

> [!NOTE]
> **Evolution**: Sentinel has evolved from a simple cleaner into a proactive **Workspace Manager**. It handles the lifecycle of your projects: **Creation**, **Management**, and **Archival**.

## 🌟 Manager Capabilities (Core)

### 🚩 CTF Logistics (Structural Mastery)
Perfectly organized workspaces, zero manual setup.

#### 1. � Smart Import & Categorization
Downloaded `chall.zip`? Don't unarchive it manually.
```bash
sentinel ctf import ~/Downloads/chall.zip
```
*   **Analysis**: Sentinel scans the zip contents before extracting.
*   **auto-Route**:
    *   Contains `Dockerfile`? -> Suggests: `Web`
    *   Contains `libc.so`? -> Suggests: `Pwn`
*   **Action**: Extracts to `Current_Event/Category/ChallName/` and creates a `solve.py` template.

#### 2. 📝 Writeup Assembly
Don't let your notes rot in 10 different folders.
```bash
sentinel ctf writeup
```
*   **Aggregates**: Scans all challenge folders for `notes.md` or `README.md`.
*   **Compiles**: Generates a single `Draft_Writeup.md` with headers for each solved challenge.

#### 3. 🏗️ Adaptive Scaffolding
```bash
sentinel ctf add pwn/heap-overflow
```
*   **Templates**: Uses specific templates based on category (e.g., `pwntools` for Pwn, `requests` for Web).

---

## 🚀 Project Scaffolding
Start new projects with best practices built-in.

```bash
# Interactive Project Creation
sentinel init --type rust --name "my-api"
```

## 🔮 Visionary Features

*   **Sentinel Brain**: Local AI/Semantic search (`sentinel ask`).
*   **Flow State**: Context switching tailored to your workflow (`sentinel resume`).
*   **Knowledge Graph**: Visualize dependencies (`sentinel graph`).
*   **Ghost Archival**: Zero-space project preservation (`sentinel archive --ghost`).

---

## 🛠️ Configuration
Totally customizable templates.

```toml
[ctf.heuristics]
# Define your own rules for smart import
pwn = ["*.elf", "libc.so*", "ld-*.so"]
web = ["package.json", "app.py", "Dockerfile"]
```
