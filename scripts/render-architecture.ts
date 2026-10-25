/** Render with the sibling Aircloud Creative Studio ELK layout engine. No Studio files are changed.
 * Run: <studio>/node_modules/.bin/tsx scripts/render-architecture.ts [studio-root]
 */
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

async function main() {
  const root = process.cwd();
  const studio = path.resolve(process.argv[2] ?? '../aircloud-creative-studio');
  const { parseFlowchart } = await import(pathToFileURL(path.join(studio, 'packages/lore/src/flowchart.ts')).href);
  const { layoutDiagram, roundedPath, DIAGRAM_TYPE } = await import(pathToFileURL(path.join(studio, 'packages/lore/src/diagram-layout.ts')).href);
  const source = fs.readFileSync(path.join(root, 'docs/diagrams/architecture.mmd'), 'utf8');
  const chart = parseFlowchart(source);
  const layout = await layoutDiagram(chart, 1600, 860);
  for (const [i,a] of layout.nodes.entries()) for (const b of layout.nodes.slice(i+1)) {
    if (a.x < b.x+b.w && b.x < a.x+a.w && a.y < b.y+b.h && b.y < a.y+a.h) throw new Error(`Overlapping boxes: ${a.id}, ${b.id}`);
  }
  const xml = (s:string) => s.replace(/[&<>"']/g, c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&apos;'}[c]!));
  const colors:Record<string,string> = { UI:'#a78bfa',Core:'#80b7ff',Manager:'#f5bc6b',Files:'#72d6ac',Library:'#72d6ac',Launch:'#80b7ff',Local:'#74d4e8',Private:'#d99bfa',Viewer:'#72e4cc',Quiver:'#f5bc6b',Alternatives:'#9dacbf' };
  const scale = layout.scale;
  const parts = [`<svg xmlns="http://www.w3.org/2000/svg" width="1648" height="980" viewBox="0 0 1648 980" role="img" aria-labelledby="title desc"><title id="title">Marquee architecture</title><desc id="desc">Desktop and browser use one Rust core. A background manager prepares verified game files for the library. Named emulator runtimes run locally or through Wolf and Moonlight. Quiver supplies discovery conventions. Sunshine and Apollo are planned alternatives shown with dashed connections.</desc><rect width="1648" height="980" rx="20" fill="#0b1020"/><text x="32" y="44" fill="#f2f5ff" font-family="sans-serif" font-size="28" font-weight="700">Marquee · one library, two ways to play</text><defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10Z" fill="#91a3c0"/></marker></defs><g transform="translate(24 68)">`];
  for (const edge of layout.edges) parts.push(`<path d="${roundedPath(edge.points,12*scale)}" fill="none" stroke="#91a3c0" stroke-width="${2.2*scale}" ${chart.edges[edge.index].dashed ? `stroke-dasharray="${7*scale} ${5*scale}"` : ''} marker-end="url(#arrow)"/>`);
  for (const n of layout.nodes) {
    const color = colors[n.id] ?? '#aab8ce';
    parts.push(`<rect x="${n.x}" y="${n.y}" width="${n.w}" height="${n.h}" rx="${10*scale}" fill="#172136" stroke="${color}" ${n.id === 'Alternatives' ? 'stroke-dasharray="8 6"' : ''} stroke-width="${1.8*scale}"/>`);
    const size=DIAGRAM_TYPE.node*scale, line=size*1.25;
    for(const [i,text] of n.lines.entries()) parts.push(`<text x="${n.x+n.w/2}" y="${n.y+n.h/2+(i-(n.lines.length-1)/2)*line}" dominant-baseline="central" text-anchor="middle" fill="#eff4ff" font-family="sans-serif" font-size="${size}">${xml(text)}</text>`);
  }
  parts.push('</g><text x="32" y="950" fill="#9dacbf" font-family="sans-serif" font-size="18">Solid: implemented integration paths · Dashed: planned alternative hosts · Headless adapters: BlastEm / Xenia</text></svg>');
  fs.writeFileSync(path.join(root,'docs/diagrams/architecture.svg'),parts.join('\n'));
  fs.writeFileSync(path.join(root,'docs/diagrams/architecture.layout.json'),JSON.stringify(layout,null,2)+'\n');
  console.log(`ELK laid out ${layout.nodes.length} nodes and ${layout.edges.length} edges without box overlaps`);
}
main().catch(e=>{console.error(e);process.exitCode=1;});
