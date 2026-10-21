// Optional isolated WebKitGTK rendering benchmark; driven by probe-rendering.py.
imports.gi.versions.Gtk = '3.0';
imports.gi.versions.WebKit2 = '4.1';
const {Gtk, GLib, WebKit2: WebKit} = imports.gi;
Gtk.init(null);
const content = new WebKit.UserContentManager();
content.register_script_message_handler('probe');
let finished = false;
content.connect('script-message-received::probe', (_manager, result) => {
    print(result.get_js_value().to_string());
    finished = true;
    Gtk.main_quit();
});
const context = new WebKit.WebContext({website_data_manager: WebKit.WebsiteDataManager.new_ephemeral()});
const view = new WebKit.WebView({web_context: context, user_content_manager: content});
view.connect('web-process-terminated', (_v, reason) => { printerr('Renderer terminated: ' + reason); Gtk.main_quit(); });
const window = new Gtk.Window({title: 'Marquee isolated rendering benchmark', default_width: 1440, default_height: 900});
window.add(view);
const cards = Array.from({length:48}, (_,i) => `<div class="card"><svg viewBox="0 0 200 270"><defs><linearGradient id="sky${i}" x2="0" y2="1"><stop stop-color="hsl(${i*17%360} 70% 9%)"/><stop offset=".55" stop-color="hsl(${i*17%360} 60% 22%)"/><stop offset="1" stop-color="#b84acc"/></linearGradient><radialGradient id="glow${i}"><stop stop-color="#feef9a"/><stop offset="1" stop-color="#eb3777" stop-opacity="0"/></radialGradient></defs><rect width="200" height="270" fill="url(#sky${i})"/><circle cx="120" cy="110" r="90" fill="url(#glow${i})"/><path d="M0 270L0 180L30 120L60 190L100 150L140 210L190 140L200 270Z" fill="#19122b"/></svg><span>Game ${i}</span></div>`).join('');
const html = `<!doctype html><meta charset="utf-8"><style>body{margin:0;background:#07070d;color:#fff;overflow:hidden;font-family:sans-serif}.grid{display:grid;grid-template-columns:repeat(8,160px);gap:20px;padding:30px;will-change:transform}.card{height:220px;position:relative;border-radius:12px;overflow:hidden;filter:drop-shadow(0 8px 16px #0009)}svg{width:100%;height:100%}span{position:absolute;bottom:14px;left:14px;filter:drop-shadow(0 4px 24px #b499ee)}</style><div class="grid">${cards}</div><script>
let start,last;const deltas=[];let frames=0;const grid=document.querySelector('.grid');
function frame(t){if(start===undefined){start=last=t;}if(t-start>1000)deltas.push(t-last);last=t;frames++;grid.style.transform='translate3d(0,'+(-120+Math.sin(t/900)*110)+'px,0)';if(t-start<8000){requestAnimationFrame(frame);return;}const s=deltas.slice().sort((a,b)=>a-b);window.webkit.messageHandlers.probe.postMessage(JSON.stringify({frames,sampled_frames:s.length,elapsed_ms:t-start,median_ms:s[Math.floor(s.length*.5)],p95_ms:s[Math.floor(s.length*.95)],max_ms:s[s.length-1],over_33ms:s.filter(x=>x>33.34).length,visibility:document.visibilityState,focused:document.hasFocus(),device_pixel_ratio:devicePixelRatio}));}requestAnimationFrame(frame);
</script>`;
view.load_html(html, 'file:///marquee-isolated-render-test/');
window.set_accept_focus(false);
window.show_all();

GLib.timeout_add(GLib.PRIORITY_DEFAULT, 18000, () => { if(!finished)printerr('Renderer probe timed out'); Gtk.main_quit(); return GLib.SOURCE_REMOVE; });
Gtk.main();
window.destroy();
if(!finished)imports.system.exit(1);
