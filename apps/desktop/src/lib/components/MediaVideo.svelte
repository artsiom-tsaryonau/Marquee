<script lang="ts">
  import { onDestroy } from 'svelte';
  import { app } from '../state.svelte';
  import { music } from '../music.svelte';
  import type { MediaAsset } from '../api/contracts';
  let { asset, url, onerror, onposition }:{asset:MediaAsset;url:string;onerror:()=>void;onposition:(id:string,position:number)=>void}=$props();
  let video=$state<HTMLVideoElement>();let saved=0;let ready=false;
  function remember(){if(ready&&video&&Number.isFinite(video.currentTime)){const position=video.ended?0:video.currentTime;onposition(asset.id,position);void app.api.mediaPosition(asset.id,position).catch(()=>{});}}
  function loaded(){ready=true;if(video&&asset.position>0&&asset.position<video.duration-2){video.currentTime=asset.position;saved=asset.position;}}
  function progress(){if(video&&Math.abs(video.currentTime-saved)>=10){saved=video.currentTime;remember();}}
  onDestroy(()=>{remember();video?.pause();});
</script>
<!-- Disc extras may have no subtitle tracks; native controls remain available. -->
<!-- svelte-ignore a11y_media_has_caption -->
<video bind:this={video} src={url} controls playsinline preload="metadata" onloadedmetadata={loaded} ontimeupdate={progress} onpause={remember} onended={remember} onplay={()=>music.pause()} {onerror}></video>
<style>video{display:block;width:100%;max-height:65vh;aspect-ratio:16/9;object-fit:contain;background:#06080c;border-radius:6px}:global(.viewer:fullscreen) video{flex:1;min-height:0;max-height:none}</style>
