/**
 * Centralized path resolution for all agent-browser-session filesystem locations.
 *
 * Directory layout:
 *   ~/.agent-browser/
 *   ├── sys/                        ← IPC files (socket, port, pid, stream)
 *   │   ├── {session}.sock
 *   │   ├── {session}.port
 *   │   ├── {session}.pid
 *   │   └── {session}.stream
 *   ├── headed-profile/             ← Headed browser data (cookies, auth, etc.)
 *   │   └── {session}/
 *   └── headless-profile/           ← Headless browser data (isolated from headed)
 *       └── {session}/
 */

import * as path from 'path';
import * as os from 'os';

/**
 * Get the base directory for all agent-browser-session data.
 * Priority: AGENT_BROWSER_SOCKET_DIR > XDG_RUNTIME_DIR > ~/.agent-browser > tmpdir
 */
export function getBaseDir(): string {
  if (process.env.AGENT_BROWSER_SOCKET_DIR) {
    return process.env.AGENT_BROWSER_SOCKET_DIR;
  }

  if (process.env.XDG_RUNTIME_DIR) {
    return path.join(process.env.XDG_RUNTIME_DIR, 'agent-browser');
  }

  const homeDir = os.homedir();
  if (homeDir) {
    return path.join(homeDir, '.agent-browser');
  }

  return path.join(os.tmpdir(), 'agent-browser');
}

/**
 * Get the directory for IPC system files (socket, pid, port, stream).
 */
export function getSysDir(): string {
  return path.join(getBaseDir(), 'sys');
}

/**
 * Get the browser profile directory, scoped by session and mode.
 */
export function getProfileDir(session: string, mode: 'headed' | 'headless' = 'headed'): string {
  return path.join(getBaseDir(), `${mode}-profile`, session);
}
