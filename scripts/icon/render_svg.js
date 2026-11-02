// Renders an SVG to transparent PNG files of the given sizes with Chromium.
// Usage: node render_svg.js <svg> <output dir> <size>...
const { chromium } = require('playwright');
const fs = require('fs');
(async () => {
  const [svgPath, outDir, ...sizes] = process.argv.slice(2);
  const svg = fs.readFileSync(svgPath, 'utf8');
  const browser = await chromium.launch();
  for (const s of sizes.map(Number)) {
    const page = await browser.newPage({ viewport: { width: s, height: s } });
    const data = Buffer.from(svg).toString('base64');
    await page.setContent(`<html><body style="margin:0;background:transparent"><img src="data:image/svg+xml;base64,${data}" width="${s}" height="${s}"></body></html>`);
    await page.screenshot({ path: `${outDir}/icon-${s}.png`, omitBackground: true });
    await page.close();
  }
  await browser.close();
})();
