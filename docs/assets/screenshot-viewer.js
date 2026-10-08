// Screenshot an exported MusubiCAD HTML joint viewer with a joint moved.
// Usage: node screenshot-viewer.js viewer.html out.png [joint=radians ...]
// Needs the `playwright` package and a Chromium build (software WebGL is fine).
const { chromium } = require("playwright");

(async () => {
  const [html, output, ...joints] = process.argv.slice(2);
  const browser = await chromium.launch({
    args: ["--use-gl=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"],
  });
  const page = await browser.newPage({ viewport: { width: 960, height: 560 } });
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto("file://" + require("path").resolve(html));
  for (const assignment of joints) {
    const [name, value] = assignment.split("=");
    await page.$eval(`input[aria-label="${name}"]`, (input, radians) => {
      input.value = radians;
      input.dispatchEvent(new Event("input"));
    }, value);
  }
  await page.waitForTimeout(300);
  await page.screenshot({ path: output });
  await browser.close();
  if (errors.length) {
    console.error(errors.join("\n"));
    process.exit(1);
  }
})();
