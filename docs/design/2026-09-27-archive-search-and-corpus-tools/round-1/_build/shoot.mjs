import { createRequire } from 'module';
const require = createRequire('/Users/dominiklukes/gitrepos/06_apps-utilities/01_desktop-apps/writeflex-desktop/package.json');
const { chromium } = require('playwright');
import path from 'path'; import fs from 'fs';
const OUT = process.argv[2]; const files = process.argv.slice(3);
const shots = path.join(OUT, 'screens'); fs.mkdirSync(shots, { recursive: true });
const browser = await chromium.launch();
for (const f of files) {
  const base = path.basename(f, '.html');
  for (const theme of ['light', 'dark']) {
    const page = await browser.newPage({ viewport: { width: 1536, height: 1000 }, colorScheme: theme, deviceScaleFactor: 1 });
    await page.goto('file://' + path.join(OUT, f) + '?theme=' + theme);
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(250);
    await page.screenshot({ path: path.join(shots, `${base}--${theme}--full.png`), fullPage: true });
    const ids = await page.$$eval('[id].desk,[id].win,[id].wfwin,[id].aswin,[id].pg-row', els => els.filter(e => !e.parentElement.closest('[id].desk')).map(e => e.id));
    for (const id of ids) {
      await page.locator('#' + id).screenshot({ path: path.join(shots, `${base}--${theme}--${id}.png`) });
    }
    // overflow check: text elements whose content is wider than box (clipping), excluding intentional ellipsis/clamp containers
    const report = await page.evaluate(() => {
      const out = [];
      document.querySelectorAll('.win,.wfwin,.aswin').forEach(w => {
        const wr = w.getBoundingClientRect();
        w.querySelectorAll('*').forEach(el => {
          const r = el.getBoundingClientRect();
          if (r.width === 0) return;
          if (r.right > wr.right + 1 && getComputedStyle(el).position !== 'absolute') out.push(`${w.id}: ${el.className} extends past window right by ${Math.round(r.right - wr.right)}px :: ${el.textContent.trim().slice(0, 40)}`);
        });
      });
      return out.slice(0, 40);
    });
    if (report.length) console.log(base, theme, '\n  ' + report.join('\n  '));
    console.log('shot', base, theme, ids.length, 'frames');
    await page.close();
  }
}
await browser.close();
