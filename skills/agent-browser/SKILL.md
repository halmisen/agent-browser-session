---
name: agent-browser-session
description: Automates browser interactions for web testing, form filling, screenshots, and data extraction. Use when the user needs to navigate websites, interact with web pages, fill forms, take screenshots, test web applications, or extract information from web pages.
---

# Browser Automation with agent-browser-session

## Quick start

```bash
agent-browser-session open <url>        # Navigate to page
agent-browser-session snapshot -i       # Get interactive elements with refs
agent-browser-session click @e1         # Click element by ref
agent-browser-session fill @e2 "text"   # Fill input by ref
agent-browser-session close             # Close browser
```

## Core workflow

1. Navigate: `agent-browser-session open <url>`
2. Snapshot: `agent-browser-session snapshot -i` (returns elements with refs like `@e1`, `@e2`)
3. Interact using refs from the snapshot
4. Re-snapshot after navigation or significant DOM changes

## Commands

### Navigation
```bash
agent-browser-session open <url>      # Navigate to URL
agent-browser-session back            # Go back
agent-browser-session forward         # Go forward  
agent-browser-session reload          # Reload page
agent-browser-session close           # Close browser
```

### Snapshot (page analysis)
```bash
agent-browser-session snapshot        # Full accessibility tree
agent-browser-session snapshot -i     # Interactive elements only (recommended)
agent-browser-session snapshot -c     # Compact output
agent-browser-session snapshot -d 3   # Limit depth to 3
```

### Interactions (use @refs from snapshot)
```bash
agent-browser-session click @e1           # Click
agent-browser-session dblclick @e1        # Double-click
agent-browser-session fill @e2 "text"     # Clear and type
agent-browser-session type @e2 "text"     # Type without clearing
agent-browser-session press Enter         # Press key
agent-browser-session press Control+a     # Key combination
agent-browser-session hover @e1           # Hover
agent-browser-session check @e1           # Check checkbox
agent-browser-session uncheck @e1         # Uncheck checkbox
agent-browser-session select @e1 "value"  # Select dropdown
agent-browser-session scroll down 500     # Scroll page
agent-browser-session scrollintoview @e1  # Scroll element into view
```

### Get information
```bash
agent-browser-session get text @e1        # Get element text
agent-browser-session get value @e1       # Get input value
agent-browser-session get title           # Get page title
agent-browser-session get url             # Get current URL
```

### Screenshots
```bash
agent-browser-session screenshot          # Screenshot to stdout
agent-browser-session screenshot path.png # Save to file
agent-browser-session screenshot --full   # Full page
```

### Wait
```bash
agent-browser-session wait @e1                     # Wait for element
agent-browser-session wait 2000                    # Wait milliseconds
agent-browser-session wait --text "Success"        # Wait for text
agent-browser-session wait --load networkidle      # Wait for network idle
```

### Semantic locators (alternative to refs)
```bash
agent-browser-session find role button click --name "Submit"
agent-browser-session find text "Sign In" click
agent-browser-session find label "Email" fill "user@test.com"
```

## Example: Form submission

```bash
agent-browser-session open https://example.com/form
agent-browser-session snapshot -i
# Output shows: textbox "Email" [ref=e1], textbox "Password" [ref=e2], button "Submit" [ref=e3]

agent-browser-session fill @e1 "user@example.com"
agent-browser-session fill @e2 "password123"
agent-browser-session click @e3
agent-browser-session wait --load networkidle
agent-browser-session snapshot -i  # Check result
```

## Persistent State (automatic)

Browser state (cookies, localStorage) is automatically persisted to `~/tmp/agent-browser-session/`. No need to manually save/load state for authentication:

```bash
# Login once - state is automatically saved
agent-browser-session open https://app.example.com/login
agent-browser-session snapshot -i
agent-browser-session fill @e1 "username"
agent-browser-session fill @e2 "password"
agent-browser-session click @e3
agent-browser-session wait --url "**/dashboard"
agent-browser-session close

# Later: cookies and localStorage are preserved
agent-browser-session open https://app.example.com/dashboard  # Already logged in!
```

## Tabs (多标签页)

管理多个浏览器标签页，共享 cookies 和 localStorage：

### 基本命令
```bash
agent-browser-session tab new              # 创建新 tab
agent-browser-session tab new <url>        # 创建新 tab 并导航到 URL
agent-browser-session tab list             # 列出所有 tabs
agent-browser-session tab <n>              # 切换到第 n 个 tab (从 0 开始)
agent-browser-session tab close            # 关闭当前 tab
agent-browser-session tab close <n>        # 关闭第 n 个 tab
```

### 使用场景

**1. 保持登录状态访问多页面**
```bash
agent-browser-session open https://app.com/login
# ... 完成登录 ...
agent-browser-session tab new https://app.com/dashboard  # 共享登录态
agent-browser-session tab new https://app.com/settings   # 同样已登录
```

**2. 外部链接自动追踪**
点击 `target="_blank"` 链接时，新页面会自动被追踪：
```bash
agent-browser-session click @e1  # 点击打开新窗口的链接
agent-browser-session tab list   # 查看所有 tabs，包括新打开的
agent-browser-session tab 1      # 切换到新 tab
```

**3. 并行数据采集**
```bash
agent-browser-session open https://shop.com/list
agent-browser-session tab new https://shop.com/item/1
agent-browser-session tab new https://shop.com/item/2
agent-browser-session tab 1 && agent-browser-session snapshot -i  # 采集第一个商品
agent-browser-session tab 2 && agent-browser-session snapshot -i  # 采集第二个商品
```

## Sessions

Note: All sessions share the same userDataDir (`~/tmp/agent-browser-session/`), so parallel sessions may cause file locking issues. Use one session at a time for best results.

```bash
agent-browser-session --session mytest open site.com  # Named session
agent-browser-session session list                     # List active sessions
```

## JSON output (for parsing)

Add `--json` for machine-readable output:
```bash
agent-browser-session snapshot -i --json
agent-browser-session get text @e1 --json
```

## Debugging

```bash
agent-browser-session open example.com --headed  # Show browser window
agent-browser-session console                    # View console messages
agent-browser-session errors                     # View page errors
```
