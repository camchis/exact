// `screenshot <path> over <ms> every <ms>` (LLP 1012.001.000 D2): film, the
// frames assembled into a contact sheet or an animated PNG. `agent.mjs`'s
// session calls it from `screenshot`.
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { MalformedOperation } from './agent-keys.mjs';
import { worldTells, lookAcross, FILM_FRAMES, FILM_PIXELS } from './agent-inspect.mjs';
import { contactSheet, encodeApng, encodePng } from './png.mjs';

/** Film on the agent's clock (LLP 1012.001.000 D2): a frame, `clock +every`, a frame … through `over`. A `.apng` path is an animated PNG (for a person to play); any other is one PNG of the frames in a grid (for an agent to look at), both at the drive's `--scale` (a grid wider than 2048 px is shrunk again; the reply's `scale` is the picture's). Every frame is also kept at full size beside it, `<path>.frames/<i>-<clock>ms.png`. The clock lands at the last frame. A step that fails stops the film: what was taken is written, and the error says so. */
export async function film(s, { carrier, timing, scale }, path, { over, every, window = false }) {
  const frames = Math.floor(over / every) + 1;
  if (!(Number.isFinite(over) && over >= 0 && Number.isFinite(every) && every >= 1)) throw new MalformedOperation('screenshot over <ms> every <ms>: over ≥ 0, every ≥ 1 ms');
  if (frames > FILM_FRAMES) throw new MalformedOperation(`screenshot over ${over} every ${every}: ${frames} frames, at most ${FILM_FRAMES}; take a longer every or a shorter over`);
  // Under platform timing UIKit's transitions run on their own clock: frames would not be the clock's.
  if (timing === 'platform') throw new Error('screenshot over: film is the agent clock\'s, and --timing platform leaves UIKit\'s motion on its own');
  const out = resolve(path), dir = out + '.frames', animated = /\.apng$/i.test(out), images = [], at = [];
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  let last, failure, told = [];
  for (let i = 0; i < frames && !failure; i++) {
    try {
      if (i) await s.clock('+' + every);
      const file = resolve(dir, `${String(i).padStart(3, '0')}-${s.now}ms.png`);
      // The frame on disk stays at full size (D2); the sheet takes it at the drive's --scale.
      told.push(last = await s.tagged(typeof window === 'function' ? await window(file) : await worldTells(s, await carrier.screenshot(file, window, true)))); // a function: a world's look frame; a live frame's tells (LLP 1127.000 D1)
      const { image } = s.scaled(last);
      if (scale !== undefined) last = { ...last, scale };
      if (images.length && (image.width !== images[0].width || image.height !== images[0].height)) throw new Error(`frame ${i} is ${image.width}×${image.height}, frame 0 ${images[0].width}×${images[0].height}; film needs one size`);
      if (frames * image.width * image.height > FILM_PIXELS) throw new Error(`${frames} frames of ${image.width}×${image.height} px exceed ${FILM_PIXELS / 1e6}M pixels; take a longer every, a shorter over, or a smaller --size`);
      images.push(image); at.push(s.now);
    } catch (error) { failure = Object.assign(new Error(`screenshot over: stopped at frame ${i} (clock ${s.now}) of ${frames}: ${error.message}`), { reply: error.reply }); }
  }
  const sheet = images.length && !animated ? contactSheet(images) : null;
  if (images.length) writeFileSync(out, animated ? encodeApng(images, every) : encodePng(sheet));
  else rmSync(dir, { recursive: true, force: true });
  if (failure) { failure.message += images.length ? `; the ${images.length} frames taken are in ${out} and ${dir}` : ''; throw failure; }
  const { screenshot, ...tags } = last;
  // `scale` is the written picture's pixels a point: a sheet wider than its bound is shrunk again, by a whole factor.
  const perPoint = (sheet ? sheet.cell[0] : images[0].width) / tags.w;
  return { ...tags, ...lookAcross(told), ...(Number.isFinite(perPoint) ? { scale: Math.round(perPoint * 1000) / 1000 } : {}), screenshot: out, frames, every, over, at, dir, form: animated ? 'animated' : 'sheet' };
}
