import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import * as path from 'path';
import * as os from 'os';
import { getBaseDir, getSysDir, getProfileDir } from './paths.js';

describe('paths', () => {
  const originalEnv = { ...process.env };

  beforeEach(() => {
    delete process.env.AGENT_BROWSER_SOCKET_DIR;
    delete process.env.XDG_RUNTIME_DIR;
  });

  afterEach(() => {
    process.env = { ...originalEnv };
  });

  describe('getBaseDir', () => {
    it('should use AGENT_BROWSER_SOCKET_DIR if set', () => {
      process.env.AGENT_BROWSER_SOCKET_DIR = '/custom/dir';
      expect(getBaseDir()).toBe('/custom/dir');
    });

    it('should use XDG_RUNTIME_DIR if set', () => {
      process.env.XDG_RUNTIME_DIR = '/run/user/1000';
      expect(getBaseDir()).toBe('/run/user/1000/agent-browser');
    });

    it('should default to ~/.agent-browser', () => {
      expect(getBaseDir()).toBe(path.join(os.homedir(), '.agent-browser'));
    });
  });

  describe('getSysDir', () => {
    it('should return sys/ under base dir', () => {
      expect(getSysDir()).toBe(path.join(os.homedir(), '.agent-browser', 'sys'));
    });

    it('should respect AGENT_BROWSER_SOCKET_DIR', () => {
      process.env.AGENT_BROWSER_SOCKET_DIR = '/custom';
      expect(getSysDir()).toBe('/custom/sys');
    });
  });

  describe('getProfileDir', () => {
    it('should return headed profile path by default', () => {
      const result = getProfileDir('test-sess');
      expect(result).toBe(path.join(os.homedir(), '.agent-browser', 'headed-profile', 'test-sess'));
    });

    it('should return headed profile path explicitly', () => {
      const result = getProfileDir('test-sess', 'headed');
      expect(result).toBe(path.join(os.homedir(), '.agent-browser', 'headed-profile', 'test-sess'));
    });

    it('should return headless profile path', () => {
      const result = getProfileDir('test-sess', 'headless');
      expect(result).toBe(
        path.join(os.homedir(), '.agent-browser', 'headless-profile', 'test-sess')
      );
    });

    it('should isolate headed and headless profiles', () => {
      const headed = getProfileDir('test-sess', 'headed');
      const headless = getProfileDir('test-sess', 'headless');
      expect(headed).not.toBe(headless);
    });
  });
});
