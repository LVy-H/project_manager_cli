# Wardex: Your Second Brain for Development

> [!NOTE]
> **Evolution**: Wardex has evolved from a simple cleaner into a proactive **Workspace Manager**. It handles the lifecycle of your projects: **Creation**, **Management**, and **Archival**.

## 🌟 Manager Capabilities (Core)

### 🚩 CTF Logistics (Structural Mastery)
Perfectly organized workspaces, zero manual setup.

#### 1. � Smart Import & Categorization
Downloaded `chall.zip`? Don't unarchive it manually.
```bash
#### 1. ⚡ Smart Import & Categorization
Downloaded `chall.zip`? Don't unarchive it manually.
```bash
wardex ctf import ~/Downloads/chall.zip
```
*   **Analysis**: Detects category from both filename and archive contents.
*   **auto-Route**:
    *   **Filename**: Contains `web`/`pwn`/`crypto`/`rev`/`misc`? → Corresponding category
    *   **Contents** (if filename doesn't match):
        *   Contains `Dockerfile`, `package.json`, `app.py`? → Category: `web`
        *   Contains `libc.so`, `.elf`, `ld-`? → Category: `pwn`
        *   Contains `crypto`, `cipher`, `rsa`, `aes`? → Category: `crypto`
        *   Contains `.exe`, `.dll`? → Category: `rev`
    *   **Default**: `misc` if no match
*   **Action**: Copies archive to `Current_Event/Category/ChallName/` and creates a category-specific solve script template.

#### 2. 📝 Writeup Assembly
Don't let your notes rot in 10 different folders.
```bash
wardex ctf writeup
```
*   **Aggregates**: Scans all challenge folders for `notes.md` or `README.md`.
*   **Compiles**: Generates a single `Draft_Writeup.md` with headers for each solved challenge.

#### 3. 🏗️ Adaptive Scaffolding
```bash
wardex ctf add pwn/heap-overflow
```
*   **Templates**: Uses specific templates based on category (e.g., `pwntools` for Pwn, `requests` for Web).

---

## 🚀 Project Scaffolding
Start new projects with best practices built-in.

```bash
# Interactive Project Creation
wardex init --type rust --name "my-api"
```

## 🔮 Roadmap / Future Features

> [!WARNING]
> **These features are planned but NOT YET IMPLEMENTED:**

*   **Wardex Brain**: Local AI/Semantic search (`wardex ask`) - *Not implemented*
*   **Flow State**: Context switching tailored to your workflow (`wardex resume`) - *Not implemented*
*   **Knowledge Graph**: Visualize dependencies (`wardex graph`) - *Not implemented*
*   **Ghost Archival**: Zero-space project preservation (`wardex archive --ghost`) - *Not implemented*

---

## 🛠️ Configuration
Totally customizable templates.

```toml
[ctf.heuristics]
# Define your own rules for smart import
pwn = ["*.elf", "libc.so*", "ld-*.so"]
web = ["package.json", "app.py", "Dockerfile"]
```
