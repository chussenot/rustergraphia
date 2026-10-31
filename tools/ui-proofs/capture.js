// Captures the same pages from the Ruby original and the Rust clone, for
// docs/ui-proofs.md: every page is reached the same way on both servers
// (the same plate file posted to /decode, the same chapter date), in a
// fresh browser context with reduced motion so the wedges are pressed at
// once, and screenshotted full page.
//
// Usage: node capture.js RUBY_BASE RUST_BASE PLATES_DIR OUT_DIR
const fs = require('fs');
const path = require('path');
const { chromium } = require('playwright');

const [rubyBase, rustBase, platesDir, outDir] = process.argv.slice(2);
const KEY = 'K7Q2M-ZXB4P';
const plate = (name) => fs.readFileSync(path.join(platesDir, `${name}.svg`), 'utf8');

// name, how to reach it, options
const PAGES = [
  { name: 'inscribe', get: '/' },
  { name: 'chapter', get: '/chapter' },
  { name: 'read-form', get: '/decode' },
  { name: 'desk-phaistos', post: { svg: 'intact' } },
  { name: 'desk-rongorongo', post: { svg: 'rongorongo' } },
  { name: 'desk-grille', post: { svg: 'grille', grille: KEY } },
  { name: 'desk-pathless', post: { svg: 'pathless' } },
  { name: 'notebook-own-tablet', post: { svg: 'tablet' } },
  { name: 'bench-weathered', post: { svg: 'worn' } },
  { name: 'bench-tallied', post: { svg: 'tallied' } },
  { name: 'board-broken', post: { svg: 'broken' } },
  { name: 'board-ruined', post: { svg: 'ruined' } },
  { name: 'decoded-edition', post: { svg: 'worn', decipher: '1' } },
  { name: 'decoded-sealed', post: { svg: 'sealed', passphrase: 'monastery gate' } },
  { name: 'decoded-two-readers', post: { svg: 'two_readers', grille: KEY } },
  { name: 'inscribe-dark', get: '/', dark: true },
  // Inscribing draws new random splits on each server: the page around the
  // plate must match, the plate itself cannot
  { name: 'inscribed-result', inscribe: 'Meet me at the old abbey after vespers' },
];

async function open(browser, base, spec) {
  const context = await browser.newContext({
    viewport: { width: 1280, height: 900 },
    reducedMotion: 'reduce',
    colorScheme: spec.dark ? 'dark' : 'light',
  });
  const page = await context.newPage();
  if (spec.get) {
    await page.goto(base + spec.get, { waitUntil: 'networkidle' });
  } else if (spec.inscribe) {
    await page.goto(base + '/', { waitUntil: 'networkidle' });
    await page.fill('#message', spec.inscribe);
    await Promise.all([
      page.waitForNavigation({ waitUntil: 'networkidle' }),
      page.evaluate(() => document.querySelector('#message').form.submit()),
    ]);
  } else {
    await page.goto(base + '/decode', { waitUntil: 'networkidle' });
    const fields = { ...spec.post, svg: plate(spec.post.svg) };
    await Promise.all([
      page.waitForNavigation({ waitUntil: 'networkidle' }),
      page.evaluate((fields) => {
        const form = document.createElement('form');
        form.method = 'post';
        form.action = '/decode';
        for (const [name, value] of Object.entries(fields)) {
          const input = document.createElement('textarea');
          input.name = name;
          input.value = value;
          form.appendChild(input);
        }
        document.body.appendChild(form);
        form.submit();
      }, fields),
    ]);
  }
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(400);
  return { context, page };
}

(async () => {
  fs.mkdirSync(outDir, { recursive: true });
  const browser = await chromium.launch();
  for (const spec of PAGES) {
    for (const [side, base] of [['ruby', rubyBase], ['rust', rustBase]]) {
      const { context, page } = await open(browser, base, spec);
      await page.screenshot({ path: path.join(outDir, `${spec.name}.${side}.png`), fullPage: true });
      await context.close();
    }
    console.log('captured', spec.name);
  }
  await browser.close();
})();
