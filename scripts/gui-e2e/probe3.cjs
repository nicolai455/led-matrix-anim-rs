#!/usr/bin/env node
'use strict';
const { Simulator, launchChromium, baselineExpectations, sleep } = require('./harness.cjs');

(async () => {
  const sim = await new Simulator().start();
  const browser = await launchChromium();
  const page = baselineExpectations(await browser.newPage(sim.base));
  try {
    await page.waitForSnapshot('Array.isArray(state.playlist?.entries)', 8000);
    await page.evaluate(`(() => {
      window.__dirtyTrace = [];
      const original = markPlaylistDraftDirty;
      window.markPlaylistDraftDirty = function() { window.__dirtyTrace.push(new Error('dirty').stack.split('\\n').slice(1,5).join(' | ')); return original.apply(this, arguments); };
    })()`);
    await page.openDetails('#playlistDisclosure');
    await page.click('#playlistNewBtn');
    await page.setValue('#playlistNameInput', 'E2E queue');
    const firstId = await page.evaluate('catalog[0].id');
    await page.setValue('#playlistEffectSelect', firstId);
    await page.setValue('#playlistDurationInput', '5000');
    await page.click('#playlistAddBtn');
    await page.click('#playlistApplyBtn');
    await page.waitFor('state.playlistCollection.playlists.length === 2', 8000);
    await sleep(1500);
    console.log('dirty:', await page.evaluate('playlistDraftDirty'));
    console.log(await page.evaluate('window.__dirtyTrace'));
    console.log('loop checked:', await page.evaluate('document.getElementById("playlistLoop").checked'));
    console.log('loopDraft:', await page.evaluate('playlistLoopDraft'));
  } finally {
    page.close();
    browser.stop();
    await sim.stop();
  }
})().catch(error => { console.error(error); process.exit(1); });
