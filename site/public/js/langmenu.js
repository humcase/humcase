// The language menu in the bar: our own menu (never the system's), the current language checked.
// The links are the pages of each language; the section the visitor is reading travels with them.
(() => {
  const root = document.querySelector('.langmenu');
  if (!root) return;
  const btn = root.querySelector('.langmenu__btn'), list = root.querySelector('.langmenu__list');
  const items = [...list.querySelectorAll('a')];
  const open = on => {
    list.hidden = !on; btn.setAttribute('aria-expanded', String(on));
    if (on) (items.find(a => a.getAttribute('aria-checked') === 'true') || items[0]).focus();
  };
  btn.addEventListener('click', () => open(list.hidden));
  document.addEventListener('click', e => { if (!root.contains(e.target)) open(false); });
  for (const a of items) a.addEventListener('click', () => { if (location.hash) a.href = a.pathname + location.hash; });
  root.addEventListener('keydown', e => {
    if (e.key === 'Escape' && !list.hidden) { open(false); btn.focus(); }
    if (!list.hidden && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
      e.preventDefault();
      const i = items.indexOf(document.activeElement);
      items[(i + (e.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length].focus();
    }
  });
})();
