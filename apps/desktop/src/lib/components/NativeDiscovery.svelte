<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { platformLabel } from '../labels';
  import { portStatusLabel } from '../presentation';
  import type { PortEntry } from '../types';
  import type { AppListEntry } from '../api/contracts';
  import PlatformIcon from './PlatformIcon.svelte';
  import Icon from './Icon.svelte';
  import AppListImport from './AppListImport.svelte';
  let query = $state('');
  let platform = $state('');
  let imported = $state<AppListEntry[]>([]);
  let error = $state('');
  let busy = $state(false);
  const match = (value: string) => value.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase());
  const platforms = $derived([...new Set(app.ports.map(p => p.platform))]);
  const ports = $derived(app.ports.filter(p => (!platform || p.platform === platform) && match(`${p.game} ${p.name} ${p.family} ${p.requirement}`)));
  const additional = $derived(imported.filter(e => !e.recipe_id && match(`${e.display_name} ${e.project ?? ''} ${e.tags.join(' ')} ${e.source}`)));
  async function refresh() {
    if (busy) return;
    busy = true; error = '';
    try { imported = await app.api.appListEntries(); }
    catch (e) { error = (e as {message?: string}).message ?? 'Could not read imported projects'; }
    finally { busy = false; }
  }
  async function saveProject(port: PortEntry) {
    if (busy) return;
    busy = true; error = '';
    try {
      const review = await app.api.previewAppList('marquee managed catalogue', JSON.stringify({apps:[{name:port.game,project:port.name,repository:port.upstream,folderName:port.id,tags:[port.platform]}]}));
      await app.api.approveAppList(review.token, review.entries.map(e => e.id));
      imported = await app.api.appListEntries();
    } catch (e) { error = (e as {message?:string}).message ?? 'Could not save project'; }
    finally { busy = false; }
  }
  onMount(() => { void refresh(); });
</script>
<section class="native-discovery" aria-label="Native game discovery">
  <header><div><h2>Native games and ports</h2><p>Discover engine projects, review their requirements and install through Marquee. Original game data is separate.</p></div><button class="btn glass small" data-nav disabled={busy} onclick={refresh}><Icon name="refresh" size={16} />Refresh app lists</button></header>
  <div class="filters">
    <label class="search"><Icon name="search" size={18} /><input data-nav aria-label="Search native games and ports" bind:value={query} placeholder="Search games, projects or requirements…" /></label>
    <label>Original platform<select class="ui-select" data-nav bind:value={platform}><option value="">All platforms</option>{#each platforms as p}<option value={p}>{platformLabel[p]}</option>{/each}</select></label>
  </div>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <div class="projects">
    {#each ports as port (port.id)}
      <article class="glass-panel">
        <div class="identity"><span class="console"><PlatformIcon platform={port.platform} size={32} /></span><div><span class="platform">{platformLabel[port.platform]}</span><h3>{port.game}</h3><p>{port.name} · {port.family}</p></div></div>
        <p class="requirement">{port.requirement}</p>
        <div class="save"><button class="btn glass small" data-nav disabled={busy || imported.some(e => e.recipe_id === port.id)} onclick={() => saveProject(port)}>{imported.some(e => e.recipe_id === port.id) ? 'Saved to projects' : 'Save project'}</button><span>No download until you install.</span></div>
        <footer><span class="status">{app.portBusy[port.id] ?? portStatusLabel(port)}</span><button class="btn glass small" data-nav onclick={() => app.portDialogId = port.id}>{port.status === 'not-installed' ? 'Review & install' : 'View installation'}</button></footer>
      </article>
    {:else}<p>No managed projects match this search.</p>{/each}
  </div>
  {#if additional.length && !platform}
    <h2>More from your app lists</h2><p>These projects are saved for discovery. They need a supported Marquee recipe before downloading or installing here.</p>
    <ul>{#each additional as entry (entry.id)}<li><strong>{entry.display_name}</strong><span>{entry.project ?? entry.provider} · {entry.source}</span><span class="status">{entry.status === 'unsupported' ? 'Unsupported requirements' : 'Needs a managed recipe'}</span></li>{/each}</ul>
  {/if}
  <AppListImport />
  <p class="source-note">This catalogue contains Marquee’s shipped recipes and your reviewed app lists. Online Quiver catalogue search and subscriptions are planned.</p>
</section>
<style>
  .save { display:flex; align-items:center; flex-wrap:wrap; gap:10px; } .save span { font-size:11px; color:var(--text-dim); }
  .native-discovery { padding-top:20px; } header,.filters,footer,.identity { display:flex; align-items:center; gap:16px; } header { justify-content:space-between; flex-wrap:wrap; } h2 { margin:0 0 8px; font-size:20px; } h3 { margin:4px 0; font-size:17px; } p { margin:0; color:var(--text-soft); font-size:13px; line-height:1.6; } .filters { margin:20px 0; flex-wrap:wrap; align-items:end; } label { display:grid; gap:6px; font-size:12px; color:var(--text-soft); } .search { display:flex; align-items:center; flex:1; min-width:180px; padding:10px 12px; border:1px solid var(--line); border-radius:8px; background:var(--field); } input { width:100%; min-width:0; background:transparent; border:0; color:var(--text); font:inherit; outline:none; } .projects { display:grid; grid-template-columns:repeat(auto-fit,minmax(min(100%,340px),1fr)); gap:16px; margin-bottom:24px; } article { padding:20px; border-radius:12px; display:flex; flex-direction:column; gap:16px; min-width:0; } .identity>div { min-width:0; } .console { color:var(--text-soft); display:grid; place-items:center; flex-shrink:0; width:52px; height:52px; border:1px solid var(--line); border-radius:10px; } .platform,.status,.source-note { color:var(--text-dim); font-size:11px; } .requirement { flex:1; } footer { justify-content:space-between; flex-wrap:wrap; } footer button { flex-shrink:0; } ul { list-style:none; padding:0; } li { display:flex; align-items:center; flex-wrap:wrap; gap:12px; padding:16px 0; border-bottom:1px solid var(--line); font-size:13px; } li>span { color:var(--text-soft); } .error { color:var(--bad); } .source-note { margin:16px 0; } .search:focus-within { outline:2px solid var(--focus,#8abaff); outline-offset:2px; }
</style>
