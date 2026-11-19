#!/usr/bin/env node
'use strict';
const { Simulator, launchChromium, baselineExpectations, sleep } = require('./harness.cjs');

(async () => {
  const sim = await new Simulator().start();
  const browser = await launchChromium();
  const page = baselineExpectations(await browser.newPage(sim.base));
  try {
    await page.waitForSnapshot('Array.isArray(state.playlist?.entries)', 8000);
    await page.openDetails('#playlistDisclosure');
    await page.click('#playlistNewBtn');
    await page.setValue('#playlistNameInput', 'E2E queue');
    const firstId = await page.evaluate('catalog[0].id');
    await page.setValue('#playlistEffectSelect', firstId);
    await page.setValue('#playlistDurationInput', '5000');
    await page.click('#playlistAddBtn');
    await page.click('#playlistApplyBtn');
    await page.waitFor('state.playlistCollection.playlists.length === 2', 8000);
    for (let i = 0; i < 6; i++) {
      console.log(i, JSON.stringify(await page.evaluate(`({
        dirty: playlistDraftDirty, awaiting: playlistAwaitingSnapshot,
        draftId: playlistDraftId, selDraft: playlistSelectionDraftId,
        notice: document.getElementById('playlistNotice').textContent,
        persist: playlistPersistenceNotice
      })`)));
      await sleep(400);
    }
    // default preset save
    await page.click('#playlistDefaultBtn');
    await page.click('#playlistApplyBtn');
    for (let i = 0; i < 5; i++) {
      console.log('preset', i, JSON.stringify(await page.evaluate(`({
        dirty: playlistDraftDirty, awaiting: playlistAwaitingSnapshot,
        notice: document.getElementById('playlistNotice').textContent
      })`)));
      await sleep(400);
    }
    console.log('start disabled with preset draft:', await page.disabled('#playlistStartBtn'));
    await page.click('#playlistStartBtn');
    await sleep(800);
    console.log('after preset start:', JSON.stringify(await page.evaluate('({status: state.playlist.status, n: state.playlist.entries.length, notice: document.getElementById("playlistNotice").textContent})')));
    console.log('http:', page.httpErrors);
  } finally {
    page.close();
    browser.stop();
    await sim.stop();
  }
})().catch(error => { console.error(error); process.exit(1); });
