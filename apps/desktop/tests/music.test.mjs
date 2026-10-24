import assert from 'node:assert/strict';
import { test } from 'node:test';
import { MusicPlayer } from '../src/lib/music.svelte';
import { commands } from '../src/lib/api/core';

const album = {id:'album',title:'OST',artist:'Composer',source_url:'',tracks:[{id:'one',title:'Opening',available:true},{id:'missing',title:'Missing',available:false},{id:'two',title:'Stage',available:true}]};
function setup() {
  const player = new MusicPlayer();
  const output = {src:'',volume:.5,currentTime:0,plays:0,pauses:0,load(){},pause(){this.pauses++;},async play(){this.plays++;}};
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
