<script lang="ts">
  import { app } from '../state.svelte';
  import type { LutrisConfig } from '../api/contracts';
  import Icon from './Icon.svelte';
  let config = $state<LutrisConfig>({ enabled: true, include_owned: false, database_path: null, launcher_id: 'detected.lutris' });
  let saving = $state(false);
  let error = $state<string | null>(null);
  let editing = $state(false);
  const source = $derived(app.lutrisSource);
  $effect(() => { if (source && !editing) config = { ...source.config, enabled: true }; });
  async function save(event: SubmitEvent) {
    event.preventDefault(); saving = true; error = null;
    try { await app.configureLutris({ ...config, enabled: true, database_path: config.database_path?.trim() || null }); editing = false; }
    catch (err) { error = (err as { message?: string }).message ?? 'Could not connect to Lutris'; }
    finally { saving = false; }
  }
  async function disconnect() {
    saving = true; error = null;
    try { if (source) await app.configureLutris({ ...source.config, enabled: false }); editing = false; }
    catch (err) { error = (err as { message?: string }).message ?? 'Could not disconnect'; }
    finally { saving = false; }
  }
</script>

<section class="connection glass-panel" aria-labelledby="lutris-heading">
  <header><span class="mark"><Icon name="grid" size={24} /></span><div><span class="eyebrow">Automatically discovered source</span><h2 id="lutris-heading">Connected game libraries</h2></div><span class="badge" class:on={source?.config.enabled}>{source?.config.enabled ? 'Auto connected' : 'Not connected'}</span></header>
  <p>Marquee discovers your existing installations and synced store libraries automatically. Your existing Lutris backend keeps its working compatibility settings.</p>
  {#if source?.config.enabled}
    <div class="stats"><span><strong>{source.installed_count}</strong> marked installed</span>{#each Object.entries(source.cached_by_service) as [service, count]}<span><strong>{count}</strong> {({ gog: 'GOG', egs: 'Epic', steam: 'Steam' } as Record<string, string>)[service] ?? service} cached</span>{/each}</div>
    <p class="detail">{source.database_path} · {source.config.include_owned ? 'Installed games and cached store listings' : 'Installed games only'}</p>
    {#each source.warnings as warning}<p class="warning">{warning}</p>{/each}
  {:else}
    <p class="detail">{source?.detected_paths.length ? 'An existing library was found. Reconnect to show its games.' : 'No existing Lutris library was found. Automatic installation and store sign-in are not connected yet.'} Existing cached libraries need no additional sign-in.</p>
  {/if}
  {#if app.lutrisError}<p class="error" role="alert">{app.lutrisError}</p>{/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if editing}
    <form onsubmit={save} aria-busy={saving}>
      <label>Database path <span class="detail">leave empty for automatic detection</span><input data-nav value={config.database_path ?? ''} oninput={(e) => (config.database_path = e.currentTarget.value)} placeholder="/absolute/path/to/lutris/pga.db" disabled={saving} /></label>
      {#if source?.detected_paths.length}<div class="detected">{#each source.detected_paths as path}<button class="chip" type="button" data-nav disabled={saving} onclick={() => (config.database_path = path)}>Use {path.includes('.var/app') ? 'Flatpak' : 'native'} database</button>{/each}</div>{/if}
      <label>Launch through<select data-nav bind:value={config.launcher_id} disabled={saving}>{#if !app.launchers.some((l) => l.id === config.launcher_id)}<option value={config.launcher_id}>Lutris · configure in launchers</option>{/if}{#each app.launchers as launcher}<option value={launcher.id}>{launcher.name}{launcher.available ? '' : ' · needs setup'}</option>{/each}</select></label>
      <p class="detail">Choose the Lutris launcher that owns this database. For Flatpak Lutris, configure its Flatpak command in the launcher cards first.</p>
      <label class="check"><input type="checkbox" data-nav bind:checked={config.include_owned} disabled={saving} /> Include cached store listings that are not installed</label>
      <div class="actions"><button class="btn primary small" type="submit" data-nav disabled={saving || app.demo}>{saving ? 'Connecting…' : 'Save connection'}</button><button class="btn glass small" type="button" data-nav disabled={saving} onclick={() => (editing = false)}>Cancel</button></div>
    </form>
  {:else}
    <div class="actions">{#if !source?.config.enabled && source?.detected_paths.length}<button class="btn primary small" data-nav disabled={saving || app.demo} onclick={async () => { saving = true; try { await app.configureLutris({ ...config, enabled: true }); } catch (err) { error = (err as { message?: string }).message ?? 'Could not reconnect'; } finally { saving = false; } }}>Reconnect existing library</button>{/if}<button class="btn glass small" data-nav disabled={saving || app.demo} onclick={() => { if (source) config = { ...source.config, enabled: true }; editing = true; }}>Advanced settings</button>{#if source?.config.enabled}<button class="btn glass small" data-nav disabled={app.lutrisLoading || saving} onclick={() => app.loadLutris()}><Icon name="refresh" size={15} /> {app.lutrisLoading ? 'Refreshing…' : 'Refresh library'}</button><button class="text-button" data-nav disabled={saving} onclick={disconnect}>Disconnect</button>{/if}</div>
  {/if}
  {#if app.demo}<p class="detail">Connect local libraries in the desktop app. Demo data stays separate.</p>{/if}
</section>

<style>
  .connection { padding:26px; border-radius:20px; display:grid; gap:14px; } header { display:flex; gap:14px; align-items:center; } header div { flex:1; } h2 { margin:5px 0 0; font-size:21px; } p { margin:0; color:var(--text-soft); font-size:14px; line-height:1.6; }
  .mark { width:48px; height:48px; display:grid; place-items:center; border-radius:14px; background:var(--field); color:var(--accent); } .badge { padding:6px 10px; background:var(--field); border-radius:8px; font-size:12px; color:var(--text-dim); } .badge.on { color:var(--ok); }
  .stats { display:flex; flex-wrap:wrap; gap:18px; padding:16px 0; border-block:1px solid var(--line); font-size:13px; color:var(--text-dim); } .stats strong { color:var(--text); margin-right:5px; } .detail { font-size:12px; color:var(--text-dim); overflow-wrap:anywhere; } .actions,.detected { display:flex; flex-wrap:wrap; gap:10px; } .text-button { background:none; border:0; color:var(--text-dim); cursor:pointer; }
  form { display:grid; gap:16px; border-top:1px solid var(--line); padding-top:18px; } label { display:grid; gap:8px; font-size:13px; } input,select { min-width:0; background:var(--field); color:var(--text); border:1px solid var(--line); border-radius:9px; padding:11px; font:inherit; } .check { display:flex; align-items:center; gap:10px; } input[type=checkbox] { accent-color:var(--accent); } .error { color:var(--bad); } .warning { color:var(--warn); font-size:12px; }
</style>
