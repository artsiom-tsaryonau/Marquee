<script lang="ts">
  import Soundtrack from './Soundtrack.svelte';
  import MediaLibrary from './MediaLibrary.svelte';
  let {gameKey='',gameTitle='',library=false}:{gameKey?:string;gameTitle?:string;library?:boolean}=$props();
  const sections=['soundtrack','images','videos'] as const;
  let section=$state<typeof sections[number]>('images');
  function navigate(event:KeyboardEvent){if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;event.preventDefault();const i=sections.indexOf(section);section=event.key==='Home'?sections[0]:event.key==='End'?sections[2]:sections[(i+(event.key==='ArrowRight'?1:2))%3];(event.currentTarget as HTMLElement).querySelector<HTMLButtonElement>(`[data-section="${section}"]`)?.focus();}
</script>
<div class="media-sections" role="tablist" tabindex="-1" aria-label="Media sections" onkeydown={navigate}>{#each sections as item}<button data-nav data-section={item} id={`media-${item}-tab`} role="tab" aria-selected={section===item} aria-controls="media-section-panel" tabindex={section===item?0:-1} onclick={()=>section=item}>{item==='soundtrack'?'Soundtrack':item==='images'?'Images':'Videos'}</button>{/each}</div>
<div id="media-section-panel" role="tabpanel" aria-labelledby={`media-${section}-tab`} tabindex="0">
  {#if section==='soundtrack'}<Soundtrack {gameKey} {gameTitle} {library}/>{:else}{#key section}<MediaLibrary {gameKey} {gameTitle} {library} view={section==='images'?'image':'video'}/>{/key}{/if}
</div>
<style>.media-sections{display:flex;gap:8px;padding:18px var(--gutter) 0;max-width:1240px;margin:auto}.media-sections button{font:inherit;font-size:13px;padding:9px 16px;border:1px solid var(--line);border-radius:7px;color:var(--text-soft);background:transparent;cursor:pointer}.media-sections button[aria-selected=true]{background:var(--field-strong);color:var(--text);border-color:var(--accent)}button:focus-visible{outline:2px solid var(--accent);outline-offset:3px}</style>
