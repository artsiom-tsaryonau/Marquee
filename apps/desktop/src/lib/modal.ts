/** Contain keyboard and spatial focus, then restore the control that opened the dialog. */
export function modal(node: HTMLElement, close: () => void) {
  const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const siblings: HTMLElement[] = [];
  let branch: HTMLElement = node;
  while (branch.parentElement) {
    for (const sibling of branch.parentElement.children) {
      if (sibling !== branch && sibling instanceof HTMLElement && !sibling.inert && !sibling.hasAttribute('data-modal-dismiss')) {
        // Existing inert state belongs to the shell (for example InstallDialog).
        // Do not restore its old value over a newer reactive shell update.
        siblings.push(sibling);
        sibling.inert = true;
      }
    }
    branch = branch.parentElement;
    if (branch === document.body) break;
  }
  const controls = () => [...node.querySelectorAll<HTMLElement>('button, input, select, textarea, a[href], summary, [tabindex]')]
    .filter((el) => !el.closest('[inert]') && !el.matches(':disabled') && el.tabIndex >= 0 && el.getClientRects().length > 0);
  node.tabIndex = -1;
  let disposed = false;
  queueMicrotask(() => { if (!disposed) (controls()[0] ?? node).focus(); });
  function keydown(event: KeyboardEvent) {
    if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(); }
    if (event.key !== 'Tab') return;
    const elements = controls();
    const first = elements[0];
    const last = elements.at(-1);
    if (!first) { event.preventDefault(); node.focus(); }
    else if (event.shiftKey && (document.activeElement === first || document.activeElement === node)) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && (document.activeElement === last || document.activeElement === node)) { event.preventDefault(); first.focus(); }
  }
  node.addEventListener('keydown', keydown, true);
  return {
    update(next: () => void) { close = next; },
    destroy() {
      disposed = true;
      node.removeEventListener('keydown', keydown, true);
      for (const element of siblings) element.inert = false;
      if (opener?.isConnected && !opener.closest('[inert]')) opener.focus({ preventScroll: true });
    },
  };
}
