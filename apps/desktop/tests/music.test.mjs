import assert from 'node:assert/strict';
import { test } from 'node:test';
import { MusicPlayer, trackTitle, musicTime } from '../src/lib/music.svelte';
import { commands } from '../src/lib/api/core';
import { musicAccent, musicAccents, musicPalette, contrastRatio } from '../src/lib/musicAppearance';

const album = {id:'album',title:'OST',artist:'Composer',source_url:'',tracks:[{id:'one',title:'Opening',available:true},{id:'missing',title:'Missing',available:false},{id:'two',title:'Stage',available:true}]};
function setup() {
  const player = new MusicPlayer();
  const output = {src:'',volume:.5,muted:false,duration:120,currentTime:0,plays:0,pauses:0,loads:0,load(){this.loads++;},pause(){this.pauses++;},async play(){this.plays++;}};
  player.attach(output);
  return {player,output};
}
test('album queue skips missing tracks and advances without wrapping after the end', async () => {
  const {player,output} = setup();const api={musicUrl:async id=>`/music/${id}`};
  await player.play(api,album,'one');assert.equal(output.src,'/music/one');assert.equal(player.queue.length,2);
  await player.next(api);assert.equal(output.src,'/music/two');assert.equal(player.current.id,'two');
  await player.next(api);assert.equal(player.paused,true);assert.equal(output.plays,2);
});
test('game launch cancels in-flight music loading and does not resume later', async () => {
  const {player,output} = setup();let finish;
  const pending=player.play({musicUrl:()=>new Promise(resolve=>finish=resolve)},album,'one');
  player.pauseForGame();finish('/music/one');await pending;
  assert.equal(output.plays,0);assert.equal(player.paused,true);assert.equal(player.loading,false);
  assert.match(player.note,/Paused for gameplay/);
});
test('latest track selection wins when URL responses arrive out of order', async () => {
  const {player,output}=setup();let first;
  const api={musicUrl:id=>id==='one'?new Promise(resolve=>first=resolve):Promise.resolve('/music/two')};
  const pending=player.play(api,album,'one');await player.play(api,album,'two');first('/music/one');await pending;
  assert.equal(output.src,'/music/two');assert.equal(output.plays,1);
});
test('failure is visible, retry works, opt-out preserves playback and removing current stops it', async () => {
  const {player,output}=setup();
  await player.play({musicUrl:async()=>{throw {code:'MISSING',message:'File unavailable'};}},album,'one');
  assert.match(player.error,/File unavailable/);assert.equal(player.paused,true);
  await player.play({musicUrl:async()=>'/music/one'},album,'one');assert.equal(player.error,'');
  player.pauseOnGame=false;player.pauseForGame();assert.equal(player.paused,false);
  player.removed('one');assert.equal(player.current,undefined);assert.equal(player.paused,true);assert.ok(output.pauses>0);
});
test('shared transport keeps music separate from game ingestion and stores explicit links', async () => {
  const calls=[];const api=commands(async(cmd,args)=>calls.push([cmd,args]));
  await api.musicImport('album',['/audio/01.mp3']);await api.musicLink('game','album',false);
  assert.deepEqual(calls,[['music_import',{album_id:'album',paths:['/audio/01.mp3']}],['music_unlink',{game_key:'game',album_id:'album'}]]);
});

test('selecting the current track toggles pause and resume without resetting seek or fetching again', async () => {
  const {player,output}=setup();let requests=0;
  const api={musicUrl:async()=>{requests++;return '/music/one';}};
  await player.select(api,album,'one');output.currentTime=42;player.syncTime();
  await player.select(api,album,'one');assert.equal(player.paused,true);assert.equal(output.currentTime,42);
  await player.select(api,album,'one');assert.equal(player.paused,false);assert.equal(output.currentTime,42);
  assert.equal(requests,1);assert.equal(output.loads,1);assert.equal(output.plays,2);
});
test('seek clamps to real duration and previous restarts after three seconds', async () => {
  const {player,output}=setup();const api={musicUrl:async id=>'/music/'+id};
  await player.play(api,album,'two');output.currentTime=10;player.syncTime();
  await player.previous(api);assert.equal(player.current.id,'two');assert.equal(output.currentTime,0);
  player.seek(-5);assert.equal(output.currentTime,0);player.seek(500);assert.equal(output.currentTime,120);
  player.seek(Number.NaN);assert.equal(output.currentTime,120);
  player.seek(1);await player.previous(api);assert.equal(player.current.id,'one');
  output.duration=Number.POSITIVE_INFINITY;player.syncTime();assert.equal(player.duration,0);
});
test('volume preserves the last audible level and zero-volume unmute restores it', () => {
  const {player,output}=setup();player.setVolume(.4);player.toggleMute();assert.equal(output.muted,true);
  player.toggleMute();assert.equal(output.muted,false);assert.equal(output.volume,.4);
  player.setVolume(0);player.toggleMute();assert.equal(output.volume,.4);assert.equal(output.muted,false);
  player.setVolume(5);assert.equal(output.volume,1);player.setVolume(Number.NaN);assert.equal(output.volume,1);
});
test('display names remove only matching ordinals and time labels reject nonfinite metadata', () => {
  assert.equal(trackTitle('001 Title Theme',1),'Title Theme');assert.equal(trackTitle('002 Emerald Hill',2),'Emerald Hill');
  assert.equal(trackTitle('007 James Bond',1),'007 James Bond');assert.equal(trackTitle('2048',1),'2048');
  assert.equal(musicTime(142.9),'2:22');assert.equal(musicTime(Number.POSITIVE_INFINITY),'0:00');assert.equal(musicTime(-4),'0:00');
});

test('resuming after cancelling another track load cannot play the previous track under new metadata', async () => {
  const {player,output}=setup();let finish;
  await player.play({musicUrl:async()=>'/music/one'},album,'one');
  const pending=player.play({musicUrl:()=>new Promise(resolve=>finish=resolve)},album,'two');
  player.pause();finish('/music/two');await pending;
  assert.equal(output.src,'/music/one');
  await player.toggle({musicUrl:async()=>'/music/two'});
  assert.equal(output.src,'/music/two');assert.equal(player.current.id,'two');assert.equal(output.plays,2);
});

test('music colors validate saved values and keep icons and selected text readable in both themes', () => {
  assert.equal(musicAccent('not a color'),'#63e6be');assert.equal(musicAccent({color:'red'}),'#63e6be');
  assert.equal(musicAccent('#AABBCC'),'#aabbcc');
  for (const fill of [...musicAccents.map(p=>p.value),'#ffffff','#000000','#777777','#ff0000','#ffff00','#0000ff']) {
    for (const theme of ['light','dark']) {
      const palette=musicPalette(fill,theme);
      assert.equal(palette.fill,fill);
      assert.ok(contrastRatio(palette.fill,palette.ink)>=4.5,fill);
      assert.ok(contrastRatio(palette.text,theme==='light'?'#eef0f7':'#181826')>=4.5,`${fill}/${theme}`);
    }
  }
});

test('queue inserts next, reorders only upcoming tracks and preserves the active media', async () => {
  const {player,output}=setup();const api={musicUrl:async id=>'/music/'+id};
  await player.play(api,album,'one');
  player.enqueue(album,'one',true);
  assert.deepEqual(player.queue.map(t=>t.id),['one','one','two']);
  player.moveQueued(2,-1);assert.deepEqual(player.queue.map(t=>t.id),['one','two','one']);
  player.moveQueued(1,-1);player.removeQueued(0);
  assert.equal(player.current.id,'one');assert.equal(output.loads,1);assert.equal(output.currentTime,0);
  player.removeQueued(2);assert.equal(player.queue.length,2);
  await player.selectQueued(api,1);assert.equal(player.current.id,'two');assert.equal(output.src,'/music/two');
  await player.selectQueued(api,1);assert.equal(player.paused,true);
  await player.selectQueued(api,-1);assert.equal(player.current.id,'two');
});
test('queueing after a cleared queue selects its first track without starting playback', async () => {
  const {player,output}=setup();player.index=5;
  player.enqueue(album,'missing');assert.equal(player.queue.length,0);
  player.enqueue(album,'two',true);assert.equal(player.index,0);assert.equal(player.current.id,'two');
  assert.equal(output.plays,0);assert.equal(player.paused,true);
  await player.toggle({musicUrl:async id=>'/music/'+id});assert.equal(output.src,'/music/two');assert.equal(output.plays,1);
});
