#!/usr/bin/env node
'use strict';
// Ad-hoc probe used while developing the suite (kept out of run.cjs).
const { Simulator, launchChromium, baselineExpectations, sleep } = require('./harness.cjs');

(async () => {
  const sim = await new Simulator().start();
  const browser = await launchChromium();
  const page = baselineExpectations(await browser.newPage(sim.base));
  try {
    await page.waitForSnapshot('Array.isArray(state.playlist?.entries)', 8000);
    console.log('boot selectedId:', await page.evaluate('state.playlistCollection.selectedId'));
    console.log('boot active entries:', await page.evaluate('state.playlist.entries.length'));
    console.log('select value:', await page.value('#playlistSavedSelect'));
    console.log('start disabled:', await page.disabled('#playlistStartBtn'));
    console.log('notice:', await page.text('#playlistNotice'));
    await page.openDetails('#playlistDisclosure');
    await page.click('#playlistStartBtn');
    await page.waitFor('state.playlist.status === "running"', 8000, 'playlist running');
    console.log('after start:', await page.evaluate('({status: state.playlist.status, index: state.playlist.index, on: state.state})'));
    console.log('notice2:', await page.text('#playlistNotice'));
    // small named playlist save
    await page.click('#playlistStopBtn');
    await page.waitFor('state.playlist.status === "stopped"', 8000);
    await page.click('#playlistNewBtn');
    await page.setValue('#playlistNameInput', 'E2E queue');
    const firstId = await page.evaluate('catalog[0].id');
    const secondId = await page.evaluate('catalog[1].id');
    await page.setValue('#playlistEffectSelect', firstId);
    await page.setValue('#playlistDurationInput', '5000');
    await page.click('#playlistAddBtn');
    await page.setValue('#playlistEffectSelect', secondId);
    await page.click('#playlistAddBtn');
    console.log('draft entries:', await page.evaluate('playlistEntries.length'));
    await page.click('#playlistApplyBtn');
    await page.waitFor('state.playlistCollection.playlists.length === 2', 8000, 'two playlists');
    console.log('saved:', await page.evaluate('state.playlistCollection.playlists.map(p=>p.id)'));
    console.log('notice3:', await page.text('#playlistNotice'));
    // now the 73-entry default: save queue is expected to be rejected by the 16 KiB guard
    await page.click('#playlistDefaultBtn');
    console.log('draft after default:', await page.evaluate('playlistEntries.length'));
    await page.click('#playlistApplyBtn');
    await sleep(600);
    console.log('notice4:', await page.text('#playlistNotice'));
    await sleep(1200);
    console.log('notice5 (after polls):', await page.text('#playlistNotice'));
    console.log('http:', page.httpErrors);
    await page.screenshot('probe');
  } finally {
    page.close();
    browser.stop();
    await sim.stop();
  }
})().catch(error => { console.error(error); process.exit(1); });
