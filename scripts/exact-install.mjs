// `exact install` and `exact uninstall` (scripts/exact.mjs): a built macOS
// bundle copied where the OS looks for apps, and a shim named by the app's
// command where a shell looks for names.
import { spawnSync } from 'node:child_process';
import { accessSync, chmodSync, constants, existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';
import { appleExecutableName } from './app.mjs';

const APPLICATIONS = resolve(homedir(), 'Applications');
/** The name this app answers to on the command line (`app.command`, else its directory's name). */
export const commandOf = (app) => app.manifest.app?.command ?? app.name;
/** Where `install` puts the app. */
export const installedAt = (app) => resolve(APPLICATIONS, `${appleExecutableName(app)}.app`);

/** Where a shim goes: `EXACT_BIN_DIR`, else the first of these already on PATH, else `~/.local/bin` (made, and named in the advice). */
export function binDirectory() {
  if (process.env.EXACT_BIN_DIR) return resolve(process.env.EXACT_BIN_DIR);
  const path = (process.env.PATH ?? '').split(':').map((p) => p && resolve(p));
  const candidates = [resolve(homedir(), '.local/bin'), '/usr/local/bin', resolve(homedir(), 'bin')];
  return candidates.find((dir) => path.includes(dir) && writable(dir)) ?? candidates[0];
}

const writable = (dir) => { try { accessSync(dir, constants.W_OK); return true; } catch { return false; } };
const onPath = (dir) => (process.env.PATH ?? '').split(':').some((p) => p && resolve(p) === dir);

/** `exact install` — the app built at `bundle` where the OS looks for apps, and its name where a shell looks for names. */
export function install(app, bundle) {
  const target = installedAt(app);
  mkdirSync(APPLICATIONS, { recursive: true });
  // A copy, not a symlink: Launch Services registers what it finds at the
  // path, and a symlinked bundle registers the repo's copy — which moves,
  // and which a `cargo clean` deletes underneath the Dock.
  rmSync(target, { recursive: true, force: true });
  const copy = spawnSync('/bin/cp', ['-R', bundle, target], { stdio: 'inherit' });
  if (copy.status !== 0) process.exit(copy.status ?? 1);
  // Register it now rather than whenever the OS next rescans, so `open -a`
  // and Open With work in the same second this returns.
  spawnSync('/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister', ['-f', target], { stdio: 'ignore' });

  const command = commandOf(app);
  const dir = binDirectory();
  mkdirSync(dir, { recursive: true });
  const shim = resolve(dir, command);
  // `open` and not the executable: it hands the files to a copy that is
  // already running instead of starting a second one, and it returns to the
  // shell instead of holding it. `--args` after the files keeps a leading
  // `-` in a filename from being read as one of open's own switches.
  writeFileSync(shim, `#!/bin/sh
# ${app.displayName} — written by \`exact install ${app.name}\`. Delete it or
# run \`exact uninstall ${app.name}\` to take it away.
exec /usr/bin/open -a ${JSON.stringify(target)} ${'"$@"'}
`);
  chmodSync(shim, 0o755);

  console.log(`${app.displayName}: ${target}`);
  console.log(`${command}: ${shim}`);
  if (!onPath(dir)) console.log(`\n  ${dir} is not on your PATH. Add it:\n    echo 'export PATH="${dir}:$PATH"' >> ~/.zshrc && exec zsh`);
  else console.log(`\n  ${command} <file> opens it — in the copy already running, if there is one.`);
}

/** `exact uninstall` — both halves, and nothing else. */
export function uninstall(app) {
  const target = installedAt(app), shim = resolve(binDirectory(), commandOf(app));
  for (const path of [target, shim]) {
    if (!existsSync(path)) { console.log(`not installed: ${path}`); continue; }
    rmSync(path, { recursive: true, force: true });
    console.log(`removed ${path}`);
  }
}
