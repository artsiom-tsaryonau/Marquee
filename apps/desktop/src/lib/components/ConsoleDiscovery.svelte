<script lang="ts">
 import { app } from '../state.svelte';
 import { systemLabel } from '../labels';
 import { bytes } from '../format';
 import type { ConsoleInspection } from '../api/contracts';
 let {system}: {system:'ps4'|'ps5'|'xbox'|'xbox360'} = $props();
 let report=$state<ConsoleInspection|null>(null);
 let busy=$state(false);let error=$state('');
 async function inspect(){if(busy)return;busy=true;error='';try{const path=await app.api.pickConsoleContent();if(path){report=null;report=await app.api.inspectConsole(path,system);}}catch(e){error=(e as {message?:string}).message??'Inspection failed';}finally{busy=false;}}
</script>
<section class="console-discovery">
 <h3>{systemLabel(system)} content inspection</h3>
 <p>Read package metadata or archive headers before preparation.</p>
 <button class="btn glass small" data-nav disabled={busy} onclick={inspect}>{busy?'Inspecting…':'Inspect a file'}</button>
 {#if error}<p class="error" role="alert">{error}</p>{/if}
 {#if report}<div aria-live="polite">
  <p class="path">{report.source}</p><p>{bytes(report.bytes)} · {report.kind} · {report.role === 'unknown' ? 'Content role unchecked' : report.role}</p>
  {#if report.metadata}<h4>{report.metadata.title}</h4><p>{report.metadata.title_id} · {report.metadata.app_version??'Version unchecked'} · Category {report.metadata.category}</p>{/if}
  {#if report.xbox360}<p>Title ID {report.xbox360.title_id} · Media ID {report.xbox360.media_id} · Disc {report.xbox360.disc_number} of {report.xbox360.disc_count}</p>{/if}
  {#if report.content_id}<p class="path">{report.content_id}</p>{/if}
  {#if report.archive}<p>{report.archive.entries.toLocaleString()} headers · {report.archive.files.toLocaleString()} files · {bytes(report.archive.unpacked_bytes)} declared unpacked{report.archive.truncated ? ' · Partial inventory' : ''}</p>
    {#each report.archive.candidates as member}<p class="path">{member.path}<br />{member.role.replaceAll('_',' ')} · {bytes(member.bytes)}</p>{/each}
  {/if}
  {#each report.diagnostics as note}<p>{note}</p>{/each}
  <p>Not installed or added to Games. Launch validation is still required.</p>
 </div>{/if}
</section>
<style>
 .console-discovery { margin-top:18px;padding-top:18px;border-top:1px solid var(--line); } h3,h4 { margin:0 0 8px; } h3 { font-size:15px; } h4 { font-size:14px; } p { font-size:13px;line-height:1.6;color:var(--text-soft);margin:10px 0; } .path { overflow-wrap:anywhere;font-size:12px; } .error { color:var(--bad); }
</style>
