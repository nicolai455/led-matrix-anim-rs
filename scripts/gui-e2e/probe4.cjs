'use strict';
const { Simulator, launchChromium, baselineExpectations, sleep } = require('./harness.cjs');
(async () => {
  const sim = await new Simulator().start();
  const browser = await launchChromium();
  const page = baselineExpectations(await browser.newPage(sim.base));
  try {
    await page.waitForSnapshot('Array.isArray(state.playlist?.entries)', 8000);
    console.log('handler:', await page.evaluate("document.getElementById('playlistApplyBtn').onclick && document.getElementById('playlistApplyBtn').onclick.name"));
    await page.openDetails('#playlistDisclosure');
    await page.click('#playlistDefaultBtn');
    await page.click('#playlistApplyBtn');
    await sleep(200);
    console.log('notice0:', await page.text('#playlistNotice'));
    console.log('flags0:', await page.evaluate('({dirty: playlistDraftDirty, awaiting: playlistAwaitingSnapshot})'));
    await sleep(2000);
    console.log('notice1:', await page.text('#playlistNotice'));
    console.log('flags1:', await page.evaluate('({dirty: playlistDraftDirty, awaiting: playlistAwaitingSnapshot, persist: playlistPersistenceNotice})'));
  } finally { page.close(); browser.stop(); await sim.stop(); }
})().catch(e => { console.error(e); process.exit(1); });
