// rivas web preview ─ progressive enhancement over server-rendered HTML.
(() => {
  'use strict';

  const $ = (sel, root = document) => root.querySelector(sel);
  const $$ = (sel, root = document) => Array.from(root.querySelectorAll(sel));
  const body = document.body;
  const narrow = () => window.matchMedia('(max-width: 900px)').matches;
  const typing = () => {
    const el = document.activeElement;
    return el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable);
  };
  const icon = (name) => {
    const tpl = document.getElementById('icon-' + name);
    return tpl ? tpl.innerHTML : '';
  };
  const store = {
    get(k) { try { return localStorage.getItem(k); } catch { return null; } },
    set(k, v) { try { localStorage.setItem(k, v); } catch { /* private mode */ } },
  };

  // ── Toast ───────────────────────────────────────────────────────────────
  let toastTimer;
  function toast(text) {
    const el = $('#toast');
    el.textContent = text;
    el.classList.add('show');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => el.classList.remove('show'), 1800);
  }

  // ── Theme ───────────────────────────────────────────────────────────────
  function currentTheme() { return document.documentElement.dataset.theme; }
  function setTheme(theme, persist = true) {
    document.documentElement.dataset.theme = theme;
    if (persist) store.set('rivas-theme', theme);
    $('#theme-btn')?.setAttribute('aria-label', theme === 'dark' ? 'Switch to light theme' : 'Switch to dark theme');
  }
  function toggleTheme() {
    setTheme(currentTheme() === 'dark' ? 'light' : 'dark');
    // Diagrams bake in their colors; re-render them for the new theme.
    if ($('.markdown-body .mermaid')) refresh();
  }
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
    if (!store.get('rivas-theme')) setTheme(e.matches ? 'dark' : 'light', false);
  });

  // ── Sidebar ─────────────────────────────────────────────────────────────
  function toggleSidebar(force) {
    if (narrow()) {
      const open = force ?? !body.classList.contains('sidebar-open');
      body.classList.toggle('sidebar-open', open);
      if (open) setTimeout(() => $('#filter')?.focus({ preventScroll: true }), 50);
    } else {
      const hidden = force === undefined ? !body.classList.contains('sidebar-hidden') : !force;
      body.classList.toggle('sidebar-hidden', hidden);
      store.set('rivas-sidebar', hidden ? 'hidden' : 'shown');
    }
  }
  if (store.get('rivas-sidebar') === 'hidden') body.classList.add('sidebar-hidden');

  // ── File filter ─────────────────────────────────────────────────────────
  function applyFilter(q) {
    q = q.trim().toLowerCase();
    let shown = 0;
    $$('.tree .tree-file').forEach((a) => {
      const hit = !q || a.dataset.path.toLowerCase().includes(q);
      a.hidden = !hit;
      if (hit) shown++;
    });
    $$('.tree details').forEach((d) => {
      if (!q) { d.hidden = false; return; }
      const any = $$('.tree-file', d).some((a) => !a.hidden);
      d.hidden = !any;
      if (any) d.open = true;
    });
    const empty = $('.tree-empty');
    if (empty) empty.hidden = shown > 0;
  }

  // ── Progress ────────────────────────────────────────────────────────────
  const progress = {
    start() {
      const bar = $('#progress');
      bar.classList.add('active');
      bar.style.width = '30%';
      this.t = setTimeout(() => { bar.style.width = '75%'; }, 150);
    },
    done() {
      const bar = $('#progress');
      clearTimeout(this.t);
      bar.style.width = '100%';
      setTimeout(() => { bar.classList.remove('active'); bar.style.width = '0'; }, 250);
    },
  };

  // ── Live status ─────────────────────────────────────────────────────────
  function setLive(state, label) {
    const el = $('#live');
    if (!el) return;
    el.className = 'live hide-narrow ' + state;
    $('.label', el).textContent = label;
  }

  // ── Enhancements applied to freshly rendered markdown ───────────────────
  function slugify(text, used) {
    let base = text.toLowerCase().trim()
      .replace(/[^\p{L}\p{N}\s-]/gu, '')
      .replace(/\s+/g, '-') || 'section';
    let slug = base, n = 1;
    while (used.has(slug)) slug = `${base}-${n++}`;
    used.add(slug);
    return slug;
  }

  function enhanceHeadings(root) {
    const used = new Set();
    $$('h1, h2, h3, h4, h5, h6', root).forEach((h) => {
      if (!h.id) h.id = slugify(h.textContent, used); else used.add(h.id);
      if (!$('.anchor', h)) {
        const a = document.createElement('a');
        a.className = 'anchor';
        a.href = '#' + h.id;
        a.setAttribute('aria-label', 'Link to this section');
        a.textContent = '#';
        h.prepend(a);
      }
    });
  }

  let tocObserver;
  function buildToc(root) {
    const toc = $('#toc');
    if (!toc) return;
    const heads = $$('h2, h3', root);
    tocObserver?.disconnect();
    if (heads.length < 2) { toc.hidden = true; return; }
    toc.hidden = false;
    const links = heads.map((h) => {
      const a = document.createElement('a');
      a.href = '#' + h.id;
      a.className = 'depth-' + h.tagName[1];
      a.textContent = h.textContent.replace(/^#/, '').trim();
      return a;
    });
    toc.replaceChildren(Object.assign(document.createElement('p'), { className: 'toc-title', textContent: 'On this page' }), ...links);
    // The current section is the last heading above the top third of the
    // viewport — correct even after large jumps (search, anchors, End key).
    const scroller = $('#scroller');
    let raf = 0;
    const update = () => {
      raf = 0;
      const line = scroller.getBoundingClientRect().top + scroller.clientHeight / 3;
      let idx = 0;
      heads.forEach((h, i) => { if (h.getBoundingClientRect().top <= line) idx = i; });
      links.forEach((a, i) => a.classList.toggle('active', i === idx));
    };
    const onScroll = () => { if (!raf) raf = requestAnimationFrame(update); };
    scroller.addEventListener('scroll', onScroll, { passive: true });
    tocObserver = { disconnect: () => scroller.removeEventListener('scroll', onScroll) };
    update();
  }

  function enhanceCode(root) {
    $$('pre > code', root).forEach((code) => {
      const pre = code.parentElement;
      const lang = (code.className.match(/language-([\w+#-]+)/) || [])[1] || '';
      if (lang === 'mermaid') return;
      // Highlighting is independent of wrapping so it can be applied later,
      // once highlight.js has loaded.
      if (window.hljs && !code.dataset.highlighted) {
        try {
          if (lang && hljs.getLanguage(lang)) hljs.highlightElement(code);
          else { code.classList.add('hljs'); code.dataset.highlighted = 'yes'; }
        } catch { /* leave plain */ }
      }
      if (pre.parentElement.classList.contains('codeblock')) return;
      const wrap = document.createElement('div');
      wrap.className = 'codeblock';
      const head = document.createElement('div');
      head.className = 'codeblock-head';
      head.innerHTML = `<span>${lang || 'text'}</span><button class="btn" type="button" aria-label="Copy code">${icon('copy')}<span class="label">Copy</span></button>`;
      $('button', head).addEventListener('click', async (e) => {
        await navigator.clipboard.writeText(code.textContent);
        const btn = e.currentTarget;
        btn.innerHTML = `${icon('check')}<span class="label">Copied</span>`;
        setTimeout(() => { btn.innerHTML = `${icon('copy')}<span class="label">Copy</span>`; }, 1500);
      });
      pre.replaceWith(wrap);
      wrap.append(head, pre);
    });
  }

  function enhanceTables(root) {
    $$('table', root).forEach((t) => {
      if (t.parentElement.classList.contains('table-wrap')) return;
      const wrap = document.createElement('div');
      wrap.className = 'table-wrap';
      t.replaceWith(wrap);
      wrap.append(t);
    });
  }

  const ALERTS = { note: 'Note', tip: 'Tip', important: 'Important', warning: 'Warning', caution: 'Caution' };
  function enhanceAlerts(root) {
    $$('blockquote[class*="markdown-alert-"]', root).forEach((q) => {
      if ($('.alert-title', q)) return;
      const kind = (q.className.match(/markdown-alert-(\w+)/) || [])[1];
      const title = document.createElement('p');
      title.className = 'alert-title';
      title.innerHTML = `${icon('info')}${ALERTS[kind] || kind}`;
      q.prepend(title);
    });
  }

  function renderMath(root) {
    if (!window.renderMathInElement) return;
    try {
      renderMathInElement(root, {
        delimiters: [
          { left: '$$', right: '$$', display: true },
          { left: '\\[', right: '\\]', display: true },
          { left: '$', right: '$', display: false },
          { left: '\\(', right: '\\)', display: false },
        ],
        throwOnError: false,
      });
    } catch (e) { console.error('KaTeX:', e); }
  }

  async function renderMermaid(root) {
    const blocks = $$('pre > code.language-mermaid', root);
    if (!blocks.length || !window.mermaid) return;
    mermaid.initialize({ startOnLoad: false, theme: currentTheme() === 'dark' ? 'dark' : 'default', securityLevel: 'strict' });
    const nodes = blocks.map((code) => {
      const div = document.createElement('div');
      div.className = 'mermaid';
      div.textContent = code.textContent;
      code.parentElement.replaceWith(div);
      return div;
    });
    try { await mermaid.run({ nodes }); } catch (e) { console.error('Mermaid:', e); }
  }

  // Structure first (instant, no dependencies), then the CDN-backed passes
  // (math, diagrams, highlighting) as soon as each library is available.
  // Every pass is idempotent, so running enhance() again is always safe.
  async function enhance() {
    const root = $('#markdown-body');
    if (!root) return;
    enhanceHeadings(root);
    enhanceTables(root);
    enhanceAlerts(root);
    buildToc(root);
    setLive('busy', 'Rendering');
    renderMath(root);
    await renderMermaid(root);
    enhanceCode(root);
    const src = $('#source-code');
    if (src && window.hljs && !src.dataset.highlighted) {
      try { hljs.highlightElement(src); } catch { /* plain */ }
    }
    setLive('', 'Live');
  }

  // ── Navigation (fetch + swap, no full reload) ───────────────────────────
  async function navigate(url, { push = true, keepScroll = false } = {}) {
    if (editor.dirty && !confirm('Discard unsaved changes?')) return;
    const scroller = $('#scroller');
    const top = scroller.scrollTop;
    progress.start();
    $('.page')?.classList.add('loading');
    try {
      const res = await fetch(url, { headers: { 'X-Rivas-Partial': '1' } });
      if (!res.ok) { location.href = url; return; }
      const doc = new DOMParser().parseFromString(await res.text(), 'text/html');
      for (const id of ['markdown-body', 'source', 'crumbs']) {
        const next = doc.getElementById(id), cur = document.getElementById(id);
        if (next && cur) cur.replaceWith(next);
      }
      body.dataset.rel = doc.body.dataset.rel || '';
      body.dataset.kind = doc.body.dataset.kind || '';
      document.title = doc.title;
      if (push) history.pushState(null, '', url);
      markActive();
      if (narrow()) toggleSidebar(false);
      await enhance();
      editor.load();
      const hash = new URL(url, location.href).hash;
      if (keepScroll) scroller.scrollTop = top;
      else if (hash) document.getElementById(decodeURIComponent(hash.slice(1)))?.scrollIntoView();
      else scroller.scrollTop = 0;
    } catch (e) {
      console.error('Navigation failed:', e);
      location.href = url;
    } finally {
      $('.page')?.classList.remove('loading');
      progress.done();
    }
  }
  const refresh = () => navigate(location.href, { push: false, keepScroll: true });

  function markActive() {
    const path = decodeURIComponent(location.pathname);
    $$('.tree-file').forEach((a) => {
      const on = decodeURIComponent(a.getAttribute('href')) === path;
      a.classList.toggle('active', on);
      if (on) {
        a.setAttribute('aria-current', 'page');
        for (let d = a.closest('details'); d; d = d.parentElement.closest('details')) d.open = true;
      } else a.removeAttribute('aria-current');
    });
  }

  // ── Source view & copy ──────────────────────────────────────────────────
  function toggleSource(force) {
    const on = force ?? !body.classList.contains('view-source');
    body.classList.toggle('view-source', on);
    $('#source-btn')?.setAttribute('aria-pressed', String(on));
  }
  async function copyMarkdown() {
    const text = editor.active ? $('#editor-input').value : ($('#source-code')?.textContent ?? '');
    await navigator.clipboard.writeText(text);
    toast('Markdown copied');
  }

  // ── Editor with live preview ────────────────────────────────────────────
  const editor = {
    active: false,
    dirty: false,
    timer: null,
    get input() { return $('#editor-input'); },
    state(text, cls = '') {
      const el = $('#save-state');
      el.textContent = text;
      el.className = 'save-state ' + cls;
    },
    load() {
      if (!this.active) return;
      this.input.value = $('#source-code')?.textContent ?? '';
      $('#editor-title').textContent = body.dataset.rel || '';
      this.dirty = false;
      this.state('Saved');
    },
    toggle(force) {
      if (body.dataset.kind !== 'file') { toast('Open a Markdown file to edit it'); return; }
      const on = force ?? !this.active;
      if (!on && this.dirty && !confirm('Discard unsaved changes?')) return;
      this.active = on;
      body.classList.toggle('editing', on);
      body.classList.remove('preview-tab');
      $('#edit-btn')?.setAttribute('aria-pressed', String(on));
      if (on) {
        toggleSource(false);
        this.load();
        this.input.focus();
      } else if (this.dirty) {
        this.dirty = false;
        refresh();
      }
    },
    changed() {
      this.dirty = true;
      this.state('Unsaved', 'dirty');
      clearTimeout(this.timer);
      this.timer = setTimeout(() => this.preview(), 250);
    },
    async preview() {
      try {
        const res = await fetch('/api/render/' + encodeURI(body.dataset.rel), {
          method: 'POST',
          headers: { 'Content-Type': 'text/plain; charset=utf-8', 'X-Rivas-Render': '1' },
          body: this.input.value,
        });
        if (!res.ok) return;
        const mdBody = $('#markdown-body');
        mdBody.innerHTML = await res.text();
        await enhance();
      } catch { /* keep the last preview */ }
    },
    async save() {
      if (!this.active) return;
      this.state('Saving…');
      try {
        const res = await fetch('/api/save/' + encodeURI(body.dataset.rel), {
          method: 'PUT',
          headers: { 'Content-Type': 'text/plain; charset=utf-8', 'X-Rivas-Save': '1' },
          body: this.input.value,
        });
        if (!res.ok) throw new Error(res.status);
        this.dirty = false;
        $('#source-code').textContent = this.input.value;
        this.state('Saved');
        toast('Saved');
      } catch {
        this.state('Save failed', 'error');
      }
    },
  };

  // ── Live reload (server-sent events) ────────────────────────────────────
  function connectLive() {
    const es = new EventSource('/api/events');
    es.onopen = () => setLive('', 'Live');
    es.onerror = () => setLive('offline', 'Offline');
    es.onmessage = (e) => {
      if (e.data !== 'reload') return;
      // Never clobber text the user is typing; saving re-syncs the editor.
      if (editor.active && editor.dirty) return;
      refresh();
    };
  }

  // ── Events ──────────────────────────────────────────────────────────────
  document.addEventListener('click', (e) => {
    const act = e.target.closest('[data-action]');
    if (act) {
      const a = act.dataset.action;
      if (a === 'sidebar') toggleSidebar();
      else if (a === 'theme') toggleTheme();
      else if (a === 'source') toggleSource();
      else if (a === 'edit') editor.toggle();
      else if (a === 'copy') copyMarkdown();
      else if (a === 'save') editor.save();
      else if (a === 'shortcuts') $('#shortcuts').showModal();
      else if (a === 'close-dialog') $('#shortcuts').close();
      else if (a === 'preview-tab') body.classList.toggle('preview-tab');
      return;
    }
    const link = e.target.closest('a[href]');
    if (!link || e.metaKey || e.ctrlKey || e.shiftKey || link.target === '_blank') return;
    const href = link.getAttribute('href');
    if (href.startsWith('/view/') || href === '/') {
      e.preventDefault();
      navigate(href);
    }
  });
  $('#backdrop').addEventListener('click', () => toggleSidebar(false));
  window.addEventListener('popstate', () => navigate(location.href, { push: false }));

  const filter = $('#filter');
  filter.addEventListener('input', () => applyFilter(filter.value));
  filter.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      const first = $$('.tree .tree-file').find((a) => !a.hidden);
      if (first) { filter.blur(); navigate(first.getAttribute('href')); }
    } else if (e.key === 'Escape') {
      filter.value = '';
      applyFilter('');
      filter.blur();
    }
  });

  const input = editor.input;
  input.addEventListener('input', () => editor.changed());
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Tab' && !e.shiftKey) {
      e.preventDefault();
      input.setRangeText('  ', input.selectionStart, input.selectionEnd, 'end');
      editor.changed();
    }
  });
  // Keep the preview roughly aligned with the editor while scrolling.
  input.addEventListener('scroll', () => {
    const s = $('#scroller');
    const ratio = input.scrollTop / Math.max(1, input.scrollHeight - input.clientHeight);
    s.scrollTop = ratio * (s.scrollHeight - s.clientHeight);
  });

  window.addEventListener('keydown', (e) => {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key.toLowerCase() === 's') { e.preventDefault(); editor.save(); return; }
    if (mod && e.key.toLowerCase() === 'k') { e.preventDefault(); if (narrow()) toggleSidebar(true); filter.focus(); filter.select(); return; }
    if (e.key === 'Escape') {
      if (body.classList.contains('sidebar-open')) toggleSidebar(false);
      return;
    }
    if (typing() || mod || e.altKey) return;
    switch (e.key) {
      case '/': e.preventDefault(); if (narrow()) toggleSidebar(true); filter.focus(); break;
      case '\\': e.preventDefault(); toggleSidebar(); break;
      case 'e': e.preventDefault(); editor.toggle(); break;
      case 'r': toggleSource(); break;
      case 't': toggleTheme(); break;
      case 'c': copyMarkdown(); break;
      case '?': $('#shortcuts').showModal(); break;
    }
  });
  window.addEventListener('beforeunload', (e) => {
    if (editor.dirty) { e.preventDefault(); e.returnValue = ''; }
  });

  // ── Boot ────────────────────────────────────────────────────────────────
  setTheme(currentTheme(), false);
  markActive();
  enhance();
  connectLive();
  // Re-run once the deferred libraries (KaTeX, Mermaid, highlight.js) load.
  if (document.readyState === 'complete') enhance();
  else window.addEventListener('load', () => enhance());
})();
