/** Other processes: whether one is alive, its command line, and this process's ancestors. */
import { spawnSync } from 'node:child_process';

export const alive = (pid) => { try { process.kill(pid, 0); return true; } catch (e) { return e.code === 'EPERM'; } };
// A process's command line: `ps` has no Windows build, where CIM's Win32_Process reports it. Empty when neither answers.
export const commandLine = (pid) => (process.platform === 'win32'
  ? spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', `(Get-CimInstance Win32_Process -Filter 'ProcessId=${Number(pid)}').CommandLine`], { encoding: 'utf8' })
  : spawnSync('ps', ['-o', 'command=', '-p', String(pid)], { encoding: 'utf8' })).stdout?.trim() ?? '';

/** This process's ancestors' pids, nearest first. */
export function ancestors() {
  const out = [];
  for (let pid = process.ppid; pid > 1 && out.length < 32;) {
    out.push(String(pid));
    const parent = Number(spawnSync('ps', ['-o', 'ppid=', '-p', String(pid)], { encoding: 'utf8' }).stdout?.trim());
    if (!Number.isInteger(parent) || parent === pid) break;
    pid = parent;
  }
  return out;
}
