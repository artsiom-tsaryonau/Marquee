<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import type { EmulatorProfileStatus, EmulatorProfileConfig, EmulatorProfileOptions } from '../api/contracts';

  let status = $state<EmulatorProfileStatus | null>(null);
  let mode = $state<EmulatorProfileConfig['mode']>('recommended');
  let custom = $state<EmulatorProfileOptions>({ renderer:'Automatic', resolution_scale:1, vsync:false });
  let loading = $state(true);
  let saving = $state(false);
  let error = $state('');
  let saved = $state(false);
  let active = true;
  const selected = $derived(mode === 'recommended' && status ? status.recommended : custom);
  function receive(next: EmulatorProfileStatus) {
    status = next; mode = next.config.mode; custom = { ...next.config.custom };
  }
  async function load() {
    loading = true; error = '';
    try { const next = await app.api.emulatorProfile('duckstation'); if (active) receive(next); }
    catch (err) { if (active) error = (err as Error).message; }
    finally { if (active) loading = false; }
  }
  async function save() {
    saving = true; error = ''; saved = false;
    try {
      const next = await app.api.configureEmulatorProfile('duckstation', { mode, custom: { ...custom } });
      if (active) { receive(next); saved = true; }
    } catch (err) { if (active) error = (err as Error).message; }
    finally { if (active) saving = false; }
  }
  onMount(() => { void load(); return () => { active = false; }; });
</script>

<section class="profile">
  <h3>Settings profile</h3>
  <p>Recommended settings use the detected host and conservative defaults. Existing settings and changes you make in DuckStation are preserved.</p>
  {#if loading}<p role="status">Detecting graphics and display…</p>
  {:else if status}
    <label>Profile<select data-nav bind:value={mode} disabled={saving} onchange={() => (saved = false)}>
      <option value="recommended">Recommended for this machine</option><option value="custom">My settings</option><option value="disabled">Leave settings unchanged</option>
    </select></label>
    <div class="options">
      <label>Renderer<select data-nav value={selected.renderer} disabled={mode !== 'custom' || saving} onchange={(e) => { custom.renderer = e.currentTarget.value as EmulatorProfileOptions['renderer']; saved = false; }}>
        <option value="Automatic">Automatic</option><option value="Vulkan">Vulkan</option><option value="OpenGL">OpenGL</option><option value="Software">Software</option>
      </select></label>
      <label>Internal resolution<select data-nav value={selected.resolution_scale} disabled={mode !== 'custom' || saving} onchange={(e) => { custom.resolution_scale = Number(e.currentTarget.value); saved = false; }}>
        {#each [1,2,3,4,6,8] as scale}<option value={scale}>{scale === 1 ? 'Native (1×)' : `${scale}× native`}</option>{/each}
      </select></label>
      <label class="check"><input type="checkbox" data-nav checked={selected.vsync} disabled={mode !== 'custom' || saving} onchange={(e) => { custom.vsync = e.currentTarget.checked; saved = false; }} /> VSync</label>
    </div>
    <p>{mode === 'disabled' ? 'Stops applying the profile. Current emulator settings stay in place.' : 'Changes apply to settings Marquee still manages. Close DuckStation before saving. This preset is not a performance benchmark.'}</p>
    <button class="btn primary small" data-nav disabled={saving} onclick={save}>{saving ? 'Saving…' : 'Save profile'}</button>
    {#if saved}<p role="status">Profile saved.{status.fields.some((f) => f.preserved) ? ' Existing or manually changed values were kept.' : ''}</p>{/if}
    {#if status.fields.some((f) => f.preserved)}<p class="notice">Some settings belong to your existing DuckStation configuration. Marquee keeps those values even when you choose a different profile.</p>{/if}
    <details><summary data-nav>Detected hardware and managed settings</summary>
      <p>Session: {status.host.session}</p>
      {#each status.host.gpus as gpu}<p>{gpu.vendor} {gpu.device} · Driver: {gpu.driver}</p>{/each}
      {#each status.host.displays as display}<p>{display.name} · {display.width} × {display.height} · {display.refresh_hz.toFixed(2)} Hz · {display.source}</p>{/each}
      {#each status.host.notes as note}<p>{note}</p>{/each}
      <dl>{#each status.fields as field}<dt>{field.section} / {field.key}</dt><dd>{field.current ?? 'Emulator default'} · {field.preserved ? 'Your value preserved' : field.managed ? 'Managed by Marquee' : 'Not applied yet'}</dd>{/each}</dl>
      <p>Profile version {status.version}. <a href={status.source} target="_blank" rel="noreferrer" data-nav>Verified settings source</a></p>
      <p class="path">{status.settings_path}</p>
    </details>
  {/if}
  {#if error}<p role="alert" class="error">{error}</p>{#if !status}<button class="btn glass small" data-nav onclick={load}>Retry</button>{/if}{/if}
</section>

<style>
  .profile { display:grid; gap:12px; padding:20px 0; border-top:1px solid var(--line); }
  h3 { margin:0; font-size:18px; } p { margin:0; color:var(--text-soft); font-size:13px; line-height:1.6; }
  label { display:grid; gap:8px; font-size:13px; color:var(--text-soft); }
  select { border:1px solid var(--line); border-radius:10px; color:var(--text); background:var(--select-bg); padding:10px; font:inherit; min-width:0; }
  .options { display:grid; grid-template-columns:1fr 1fr; gap:16px; } .check { display:flex; align-items:center; gap:8px; }
  .notice { color:var(--warn); } .error { color:var(--bad); }
  details { font-size:13px; padding-top:6px; } summary { cursor:pointer; color:var(--text-soft); padding:6px 0; }
  details p { margin:10px 0; } dl { display:grid; grid-template-columns:1fr 1fr; gap:10px; } dt { color:var(--text-dim); } dd { margin:0; overflow-wrap:anywhere; }
  a { color:var(--accent); } .path { overflow-wrap:anywhere; } .btn { justify-self:start; }
  @media(max-width:600px) { .options, dl { grid-template-columns:1fr; } }
</style>
