// Runs the editor's JavaScript core against the Rust crate's golden files.
// Usage: node web/parity.mjs   (from the repo root)
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('./editor.html', import.meta.url), 'utf8');
const core = html.slice(html.indexOf('/* CORE-BEGIN'), html.indexOf('/* CORE-END */'));
const P = new Function(`${core}; return P;`)();
const ex = name => readFileSync(new URL(`../examples/${name}`, import.meta.url), 'utf8');

const fnv = img => {
  let h = 0xcbf29ce484222325n;
  for (const b of img.data) { h ^= BigInt(b); h = (h * 0x100000001b3n) & 0xffffffffffffffffn; }
  return h.toString(16).padStart(16, '0');
};

let failed = 0;
const expect = (name, got, want) => {
  const ok = got === want;
  if (!ok) failed++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${ok ? '' : `\n  got:  ${JSON.stringify(got)}\n  want: ${JSON.stringify(want)}`}`);
};

const two = ex('two_bedroom.txt');
const rep = P.check(P.Grid.parse(two));
expect('two_bedroom is valid with 4 rooms', `${rep.valid} ${rep.rooms}`, 'true 4');

const broken = P.Grid.parse(ex('broken.txt'));
expect('broken.txt has 9 issues', P.check(broken).issues.length, 9);
P.fix(broken);
expect('fix(broken.txt) == broken_fixed.txt', broken.toText(), ex('broken_fixed.txt'));

expect('generate(seed 7, 5 rooms) == gen_seed7.txt', P.generate({ rooms: 5, seed: 7 }).toText(), ex('gen_seed7.txt'));

const plan = P.parsePlan(two);
expect('render two_bedroom (canonical) fingerprint', fnv(P.render(plan)), 'a480641fd853c7c3');
expect('render two_bedroom (blueprint 24/12) fingerprint',
  fnv(P.render(plan, { tile_w: 24, elev: 12, palette: P.PALETTES.blueprint })), '63874e8874634421');

let invalid = 0;
for (let seed = 0; seed < 200; seed++) if (!P.check(P.generate({ cols: 31, rows: 15, rooms: 4, seed })).valid) invalid++;
expect('200 generated plans all valid', invalid, 0);

process.exit(failed ? 1 : 0);
