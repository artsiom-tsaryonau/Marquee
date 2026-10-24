<script lang="ts">
  import { untrack } from 'svelte';
  import { app } from '../state.svelte';
  import { asError } from '../api/core';
  import type { MetadataReview } from '../api/contracts';
  import Icon from './Icon.svelte';
  let {editionId,title}:{editionId:string;title:string}=$props();
  let query=$state(untrack(()=>title));let review=$state<MetadataReview|null>(null);let busy=$state(false);let error=$state('');let note=$state('');
  async function search(event:SubmitEvent){event.preventDefault();if(busy)return;busy=true;error='';note='';review=null;try{review=await app.api.searchGameMetadata(editionId,query);}catch(e){error=asError(e).message;}finally{busy=false;}}
  async function apply(id:number){if(!review||busy)return;busy=true;error='';try{await app.api.applyMetadataMatch(editionId,review.token,id);app.applySnapshot(await app.api.snapshot());note='Artwork match saved. Game title, platform and files were kept.';review=null;}catch(e){error=asError(e).message;}finally{busy=false;}}
</script>
<section class="metadata-search glass-panel"><h3>Search metadata and artwork</h3><p>Find an artwork identity on SteamGridDB using your saved API key. Review the title carefully: platform and edition are not verified by this provider.</p><form onsubmit={search}><label><span>Game title</span><input data-nav bind:value={query} required minlength="2" maxlength="200"/></label><button class="btn glass small" data-nav disabled={busy||query.trim().length<2}><Icon name="search" size={16}/>{busy?'Working…':'Search'}</button></form>
{#if error}<p class="error" role="alert">{error}</p>{/if}{#if note}<p role="status">{note}</p>{/if}
{#if review}<p class="provider">{review.provider}</p>{#if !review.candidates.length}<p>No matching candidates. Try another title.</p>{:else}<ul>{#each review.candidates as candidate}<li><div><strong>{candidate.title}</strong><small>Provider ID {candidate.id}</small></div><button class="btn glass small" data-nav disabled={busy} onclick={()=>apply(candidate.id)}>Use artwork match</button></li>{/each}</ul>{/if}{/if}
</section>
<style>.metadata-search{padding:20px;border-radius:10px;margin-bottom:20px}h3{margin:0 0 10px;font-size:18px}p{color:var(--text-soft);font-size:13px;line-height:1.6}form{display:flex;align-items:end;gap:10px;flex-wrap:wrap}label{display:grid;gap:6px;font-size:12px;flex:1;min-width:0}input{font:inherit;color:var(--text);background:var(--field);border:1px solid var(--line);border-radius:7px;padding:10px;min-width:0;width:100%;box-sizing:border-box}ul{list-style:none;padding:0}li{display:flex;justify-content:space-between;align-items:center;gap:14px;padding:12px 0;border-top:1px solid var(--line)}li>div{display:grid;gap:5px;min-width:0}strong{font-size:13px;overflow-wrap:anywhere}small,.provider{color:var(--text-dim);font-size:11px}.error{color:var(--danger)}@media(max-width:520px){li{align-items:start;flex-direction:column}}</style>
