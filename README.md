# agent-browser-session

A fork of [vercel-labs/agent-browser](https://github.com/vercel-labs/agent-browser) optimized for **persistent login sessions**. Uses [Patchright](https://github.com/AjaxMultiCommentary/patchright) (anti-detection Playwright fork) + system Chrome + persistent userDataDir.

## Why This Fork?

| | `agent-browser` (upstream) | `agent-browser-session` (this fork) |
|---|---|---|
| Browser | Bundled Chromium | System Chrome |
| Profile | Ephemeral (lost on close) | Persistent (`~/tmp/agent-browser/{session}/`) |
| Mode | Headless by default | Headed by default |
| Anti-detection | Playwright (detectable) | Patchright (stealth) |
| Best for | Quick scraping, no login | OAuth, login flows, session reuse |

**Both can coexist** — install upstream via `npm i -g agent-browser` and this fork separately.

## Installation

```bash
git clone https://github.com/BUNotesAI/agent-browser-session
cd agent-browser-session
pnpm install
pnpm build
pnpm build:native   # Requires Rust (https://rustup.rs)
pnpm link --global   # Makes agent-browser-session available globally
```

## Quick Start

```bash
agent-browser-session open example.com
agent-browser-session snapshot -i               # Get interactive elements with refs
agent-browser-session click @e2                  # Click by ref
agent-browser-session fill @e3 "test@example.com"
agent-browser-session screenshot page.png
agent-browser-session close
```

## Sessions & Login Persistence

Each session gets its own Chrome profile directory. The default session is `main`.

```bash
# Default: uses "main" session → ~/tmp/agent-browser/main/
agent-browser-session open https://accounts.google.com
# Login manually in the browser window...
agent-browser-session close

# Later: login state persists
agent-browser-session open https://mail.google.com  # Already logged in!
```

### Multiple Sessions

Use `--session` for parallel isolated profiles:

```bash
# Each session has its own cookies, localStorage, login state
agent-browser-session --session work open https://github.com
agent-browser-session --session personal open https://github.com

# List active sessions
agent-browser-session session list
```

**Note:** The same session name cannot run concurrently (Chrome locks the profile directory).

### Session Storage

```
~/tmp/agent-browser/
├── main/          ← default session (persistent login)
├── work/          ← custom session
└── site1/         ← another custom session
```

## Commands

All commands are identical to upstream `agent-browser`. See the [upstream README](https://github.com/vercel-labs/agent-browser#readme) for the full command reference.

### Core Commands

```bash
agent-browser-session open <url>              # Navigate to URL
agent-browser-session click <sel>             # Click element
agent-browser-session fill <sel> <text>       # Clear and fill
agent-browser-session press <key>             # Press key
agent-browser-session snapshot                # Accessibility tree with refs
agent-browser-session snapshot -i             # Interactive elements only
agent-browser-session screenshot [path]       # Take screenshot
agent-browser-session close                   # Close browser
```

### Get Info

```bash
agent-browser-session get text <sel>          # Get text content
agent-browser-session get url                 # Get current URL
agent-browser-session get title               # Get page title
```

### Wait

```bash
agent-browser-session wait <selector>         # Wait for element
agent-browser-session wait --load networkidle # Wait for network idle
agent-browser-session wait --url "**/dash"    # Wait for URL pattern
```

### Tabs

```bash
agent-browser-session tab                     # List tabs
agent-browser-session tab new [url]           # New tab
agent-browser-session tab <n>                 # Switch to tab n
agent-browser-session tab close [n]           # Close tab
```

### Cookies & Storage

```bash
agent-browser-session cookies                 # Get all cookies
agent-browser-session storage local           # Get localStorage
```

## Options

| Option | Description |
|--------|-------------|
| `--session <name>` | Use named session (default: `main`) |
| `--headed` | Show browser window (default: always headed) |
| `--bundled` | Use bundled Chrome instead of system Chrome |
| `--channel <name>` | Browser channel: `chrome`, `msedge`, `chrome-beta` |
| `--executable-path <path>` | Custom browser executable |
| `--json` | JSON output for agents |

## Rate Limiting

To be friendly to target servers, agent-browser-session adds a **5-second delay** before each navigation by default.

```bash
# Disable delay for faster testing
AGENT_BROWSER_NAV_DELAY_MS=0 agent-browser-session open example.com

# Custom delay
AGENT_BROWSER_NAV_DELAY_MS=2000 agent-browser-session open example.com
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `AGENT_BROWSER_SESSION` | Session name | `main` |
| `AGENT_BROWSER_NAV_DELAY_MS` | Navigation delay (ms) | `5000` |
| `AGENT_BROWSER_EXECUTABLE_PATH` | Custom browser path | system Chrome |
| `AGENT_BROWSER_STREAM_PORT` | WebSocket streaming port | disabled |
| `AGENT_BROWSER_SOCKET_DIR` | Override socket directory | `~/.agent-browser` |

## Architecture

```
┌─────────────────┐     Unix Socket      ┌──────────────────┐
│  Rust CLI        │◄───────────────────►│  Node.js Daemon   │
│  (fast startup)  │  ~/.agent-browser/   │  (Patchright)     │
└─────────────────┘   main.sock          └──────────────────┘
                                                   │
                                          ┌────────┴────────┐
                                          │  System Chrome   │
                                          │  ~/tmp/agent-    │
                                          │  browser/main/   │
                                          └─────────────────┘
```

- **Rust CLI** — Fast native binary, parses commands, communicates with daemon
- **Node.js Daemon** — Manages Patchright browser via persistent context
- **Socket isolation** — `~/.agent-browser/` (not `/tmp/`), avoids TMPDIR issues

## Using with Both Versions

If you have upstream `agent-browser` installed alongside:

```bash
# Quick scraping (upstream, headless, ephemeral)
agent-browser open example.com
agent-browser snapshot -i
agent-browser close

# Login-required sites (this fork, headed, persistent)
agent-browser-session open https://github.com/settings
# Already logged in from previous session!
```

## Claude Code Skill

```bash
cp -r skills/agent-browser ~/.claude/skills/
```

## License

Apache-2.0
