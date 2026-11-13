// A stream's head admitted as exact-update admits it, and the sequence the next release takes above
// everything the stream has authenticated (LLP 1030.000 D3 item 5): scripts/deploy.mjs's classifier and
// publisher read both.
import { verify } from 'node:crypto';
import { refuse, canonicalBytes, fileCard, publicKeyFromRaw, indexCard, readAssetIndex, validateRawIntegers } from './deploy-signing.mjs';
import { OriginUnavailable, sha256, streamPath } from './origin.mjs';

function strictBase64(text, length, label) {
  if (typeof text !== 'string') throw new Error(`${label} is not base64 text`);
  const decoded = Buffer.from(text, 'base64');
  if (decoded.length !== length || decoded.toString('base64') !== text) throw new Error(`${label} is not canonical base64 of ${length} bytes`);
  return decoded;
}

/** `inspectHead`, then the asset index a usable head names, read from the
 * origin into `head.assets` as its cards; a head whose index is missing or
 * not its bytes is unusable. */
export async function admitHead(origin, found, app, stream) {
  const admission = inspectHead(found, app, stream);
  if (!admission.usable) return admission;
  try {
    const card = admission.head.assets;
    const path = card && new URL(card.url, `https://origin.invalid/${streamPath(stream)}/exact.json`).pathname.slice(1);
    const assets = card ? readAssetIndex(card, await origin.get(path)) : [];
    return { ...admission, head: { ...admission.head, assets } };
  } catch (error) {
    if (error instanceof OriginUnavailable) throw error;
    return { usable: false, authenticated: false, head: found?.json ?? null, seq: null, problem: error.message || String(error) };
  }
}

/** Parse and authenticate one origin head by the same pre-download rules as
 * exact-update. An unusable head contributes no sequence: even a syntactically
 * valid number is attacker-controlled until its signature has verified. Its
 * asset index is `admitHead`'s to read. */
export function inspectHead(found, app, stream) {
  try {
    if (!Buffer.isBuffer(found?.bytes)) throw new Error('the origin returned no head bytes');
    if (found.bytes.length > 64 * 1024) throw new Error(`the envelope is ${found.bytes.length} bytes; the most is ${64 * 1024}`);
    const text = new TextDecoder('utf-8', { fatal: true }).decode(found.bytes);
    validateRawIntegers(text);
    const head = JSON.parse(text);
    if (!head || typeof head !== 'object' || Array.isArray(head)) throw new Error('the envelope is not a JSON object');
    canonicalBytes(head); // recursively rejects every inexact JSON number
    if (head.exact !== 1) throw new Error(`the envelope is exact ${head.exact ?? '(missing)'}; this binary reads exact 1`);
    if (!head.app || typeof head.app !== 'object' || Array.isArray(head.app)) throw new Error('the envelope names no app');
    if (typeof head.app.id !== 'string' || !head.app.id) throw new Error('the envelope names no nonempty app id');
    fileCard(head.plan, 'app.plan');
    if (!head.stream || typeof head.stream !== 'object' || Array.isArray(head.stream)) throw new Error('the envelope names no stream');
    if (typeof head.stream.channel !== 'string') throw new Error('the stream names no channel');
    if (typeof head.stream.compatibilityId !== 'string') throw new Error('the stream names no compatibility id');
    if (!Number.isSafeInteger(head.stream.seq) || head.stream.seq < 0) throw new Error('the stream names no exact nonnegative seq');
    if (head.assets !== undefined) indexCard(head.assets);
    if (head.sunset !== undefined) {
      if (!head.sunset || typeof head.sunset !== 'object' || Array.isArray(head.sunset)) throw new Error('the sunset card is not an object');
      if (typeof head.sunset.message !== 'string') throw new Error('the sunset card has no message');
    }
    let signature = null;
    if (head.signature !== undefined) {
      if (!head.signature || typeof head.signature !== 'object' || Array.isArray(head.signature)) throw new Error('the signature is not an object');
      if (typeof head.signature.keyId !== 'string') throw new Error('the signature names no key id');
      signature = strictBase64(head.signature.ed25519, 64, 'the signature');
    }
    if (head.app.id !== app.id) throw new Error(`the head is for ${head.app.id}; this binary is ${app.id}`);
    if (head.app.name !== app.displayName) throw new Error(`the head calls ${app.id} ${JSON.stringify(head.app.name)}, not ${JSON.stringify(app.displayName)}`);
    if (typeof head.stream.app === 'string' && head.stream.app !== app.id) throw new Error(`the head's stream is for ${head.stream.app}; this binary is ${app.id}`);
    if (head.stream.channel !== stream.channel) throw new Error(`the head is for channel ${head.stream.channel}; this binary is ${stream.channel}`);
    if (head.stream.compatibilityId !== stream.compatibilityId) throw new Error(`the head is for cohort ${head.stream.compatibilityId}; this binary is ${stream.compatibilityId}`);
    const keys = app.manifest.deploy?.signing?.keys ?? {};
    let authenticated = false;
    if (Object.keys(keys).length) {
      if (!signature) throw new Error('the head is unsigned and this binary carries keys');
      if (!Object.hasOwn(keys, head.signature.keyId)) throw new Error(`the head is signed by ${head.signature.keyId}, which this binary does not carry`);
      const key = strictBase64(keys[head.signature.keyId], 32, `the embedded key ${head.signature.keyId}`);
      if (!verify(null, canonicalBytes(head), publicKeyFromRaw(key), signature)) throw new Error(`the head's signature by ${head.signature.keyId} does not verify`);
      authenticated = true;
    }
    return { usable: true, authenticated, head, seq: head.stream.seq, problem: null };
  } catch (error) {
    return { usable: false, authenticated: false, head: found?.json ?? null, seq: null, problem: error.message || String(error) };
  }
}

/** The largest sequence authenticated inside an immutable release record.
 * The record wrapper is audit metadata; only its embedded signed envelope is
 * authority for a client's rollback floor. Malformed, foreign, and unsigned
 * records do not contribute a number. */
async function authenticatedReleaseFloor(origin, app, stream) {
  // Hidden names were accepted by older publishers. Include them while
  // recovering the rollback floor even though new release ids cannot begin
  // with a dot.
  const names = await origin.list(`${streamPath(stream)}/releases`, { includeHidden: true });
  // A missing directory on the writable filesystem origin is authoritative
  // emptiness. Other adapters use null when they cannot enumerate history;
  // that is not evidence that no client has observed a higher sequence.
  if (names === null && origin.kind !== 'directory') return { known: false, empty: false, floor: null };
  if (names === null || names.length === 0) return { known: true, empty: true, floor: null };
  let floor = null;
  for (const name of names.filter((entry) => entry.endsWith('.json'))) {
    const bytes = await origin.get(`${streamPath(stream)}/releases/${name}`);
    if (!bytes) throw new OriginUnavailable(`the listed release record ${origin.describe()}/${streamPath(stream)}/releases/${name} disappeared while establishing the authenticated sequence floor`);
    try {
      const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
      const record = JSON.parse(text);
      if (!record.envelope || typeof record.envelope !== 'object' || Array.isArray(record.envelope)) continue;
      const recordApp = { ...app, displayName: record.envelope.app?.name };
      const envelopeBytes = Buffer.from(JSON.stringify(record.envelope), 'utf8');
      const admission = inspectHead({ bytes: envelopeBytes, json: record.envelope, sha256: sha256(envelopeBytes) }, recordApp, stream);
      if (admission.usable && admission.authenticated) floor = Math.max(floor ?? 0, admission.seq);
    } catch { /* an unauthenticated audit record has no say in the floor */ }
  }
  return { known: true, empty: false, floor };
}

/** Allocate above everything the stream has authenticated: an admitted head
 * and the maximum signed immutable history. A head rolled back on the origin
 * (a restored backup, a stale replica) would otherwise hand out a seq that
 * clients already hold for another bundle. Repairing an unusable head uses
 * the history alone; if none exists, overwriting would guess at the clients'
 * rollback floor and is refused. */
export async function nextSeq(origin, app, stream, admission, at, above = 0) {
  return Math.max(above + 1, await nextOriginSeq(origin, app, stream, admission, at));
}

async function nextOriginSeq(origin, app, stream, admission, at) {
  const history = await authenticatedReleaseFloor(origin, app, stream);
  let floor;
  if (admission?.usable) floor = Math.max(admission.seq, history.floor ?? 0);
  else {
    if (!history.known) throw new OriginUnavailable(`${at} ${admission ? `is unusable (${admission.problem})` : 'is missing'}, but ${origin.describe()} cannot enumerate its authenticated release history; a rollback-safe sequence cannot be allocated`);
    floor = history.floor;
    if (floor === null && !admission && history.empty) return 1;
    if (floor === null) refuse(`${at} ${admission ? `is unusable (${admission.problem}) and its` : 'is missing, and its nonempty'} release history has no authenticated sequence floor; restore a signed release record before repairing it`);
  }
  if (floor === Number.MAX_SAFE_INTEGER) refuse(`${at} is at the largest exact JavaScript seq; a higher repair seq cannot be allocated safely`);
  return floor + 1;
}

/** The latest release record under a stream, when the origin has any: it names the platform and carries the cohort's inputs, so two ids that differ are explained field by field. */
export async function latestRecord(origin, app, stream) {
  const names = await origin.list(`${streamPath(stream)}/releases`, { includeHidden: true });
  if (!names?.length) return null;
  const records = [];
  for (const name of names.filter((n) => n.endsWith('.json'))) {
    const bytes = await origin.get(`${streamPath(stream)}/releases/${name}`);
    try {
      const record = JSON.parse(bytes.toString('utf8'));
      const recordApp = {...app, displayName:record.envelope?.app?.name};
      const admission = inspectHead({bytes:Buffer.from(JSON.stringify(record.envelope))}, recordApp, stream);
      if(admission.usable && admission.authenticated) records.push(record);
    } catch { /* unauthenticated audit metadata has no capability authority */ }
  }
  records.sort((a,b)=>a.envelope.stream.seq-b.envelope.stream.seq);
  return records.at(-1) ?? null;
}
