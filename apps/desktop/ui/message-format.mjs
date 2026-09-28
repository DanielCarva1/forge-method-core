// Readable structure for completed Codex text. Agent URLs never navigate the app.
const previewExtension = /\.(?:txt|md|json|csv|html?|css|js|mjs|ts|rs|py|ya?ml|toml|png|jpe?g|gif|webp|pdf|docx?|xlsx?|pptx?|zip|mp[34]|wav)$/i;
function localPreviewPath(value) {
  const path = value.trim();
  if (!path || path.length > 1024 || /[\0<>"|?*#]/.test(path) || !previewExtension.test(path)) return null;
  const windowsAbsolute = /^[A-Za-z]:[\\/]/.test(path);
  if (!windowsAbsolute && (/^[\\/]/.test(path) || /^[A-Za-z][A-Za-z0-9+.-]*:/.test(path))) return null;
  return path;
}

function fileAction(path, label, onLocalFile, inlineCode = false) {
  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'message-file-link';
  button.dataset.previewPath = path;
  button.setAttribute('aria-label', `Ver arquivo local: ${label}`);
  if (inlineCode) {
    const code = document.createElement('code');
    code.textContent = label;
    button.append(code);
  } else button.textContent = label;
  button.addEventListener('click', () => onLocalFile(path));
  return button;
}

function appendInline(parent, value, onLocalFile) {
  const tokens = /\*\*[^*\n]+\*\*|`[^`\n]+`|\[[^\]\n]{1,120}\]\([^)\n]{1,1024}\)/g;
  let offset = 0;
  let match;
  let formatted = false;
  while ((match = tokens.exec(value))) {
    parent.append(document.createTextNode(value.slice(offset, match.index)));
    if (match[0].startsWith('[')) {
      const separator = match[0].indexOf('](');
      const label = match[0].slice(1, separator);
      const path = localPreviewPath(match[0].slice(separator + 2, -1));
      if (path && onLocalFile) {
        parent.append(fileAction(path, label, onLocalFile));
        formatted = true;
      } else parent.append(document.createTextNode(match[0]));
    } else {
      const code = match[0].startsWith('`');
      const label = match[0].slice(code ? 1 : 2, code ? -1 : -2);
      const path = code && onLocalFile ? localPreviewPath(label) : null;
      if (path) parent.append(fileAction(path, label, onLocalFile, true));
      else {
        const element = document.createElement(code ? 'code' : 'strong');
        element.textContent = label;
        parent.append(element);
      }
      formatted = true;
    }
    offset = tokens.lastIndex;
  }
  parent.append(document.createTextNode(value.slice(offset)));
  return formatted;
}

function tableCells(line) {
  let value = line.trim();
  if (!value.includes('|')) return null;
  if (value.startsWith('|')) value = value.slice(1);
  if (value.endsWith('|')) value = value.slice(0, -1);
  const cells = [];
  let cell = '';
  let code = false;
  for (let i = 0; i < value.length; i += 1) {
    if (value[i] === '\\' && value[i + 1] === '|') { cell += '|'; i += 1; }
    else if (value[i] === '`') { code = !code; cell += value[i]; }
    else if (value[i] === '|' && !code) { cells.push(cell.trim()); cell = ''; }
    else cell += value[i];
  }
  cells.push(cell.trim());
  return cells;
}

function isTableAt(lines, index) {
  const header = tableCells(lines[index]);
  const divider = index + 1 < lines.length ? tableCells(lines[index + 1]) : null;
  return header && divider && header.length >= 2 && header.length <= 8
    && divider.length === header.length
    && divider.every(cell => /^:?-{3,}:?$/.test(cell)) ? header : null;
}

export function renderAgentMessage(target, raw, onLocalFile) {
  const lines = raw.replace(/\r\n?/g, '\n').split('\n');
  const fragment = document.createDocumentFragment();
  let paragraph = [];
  let list = null;
  let formatted = false;
  let firstHeadingDepth = null;
  let previousHeadingLevel = 2;
  function flushParagraph() {
    if (!paragraph.length) return;
    const node = document.createElement('p');
    formatted = appendInline(node, paragraph.join('\n'), onLocalFile) || formatted;
    fragment.append(node);
    paragraph = [];
  }
  function closeList() { list = null; }

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    const fence = line.match(/^ {0,3}(`{3,}|~{3,})(.*)$/);
    if (fence) {
      flushParagraph(); closeList(); formatted = true;
      const marker = fence[1][0];
      const close = new RegExp(`^ {0,3}${marker}{${fence[1].length},}[ \\t]*$`);
      const codeLines = [];
      while (i + 1 < lines.length && !close.test(lines[i + 1])) codeLines.push(lines[++i]);
      if (i + 1 < lines.length) i += 1;
      const pre = document.createElement('pre');
      const code = document.createElement('code');
      code.textContent = codeLines.join('\n');
      pre.append(code);
      fragment.append(pre);
      continue;
    }
    if (!line.trim()) { flushParagraph(); closeList(); continue; }
    const header = isTableAt(lines, i);
    if (header) {
      flushParagraph(); closeList(); formatted = true;
      const scroll = document.createElement('div');
      scroll.className = 'message-table-scroll';
      scroll.tabIndex = 0;
      scroll.setAttribute('role', 'region');
      scroll.setAttribute('aria-label', 'Tabela da resposta');
      const table = document.createElement('table');
      const head = document.createElement('thead');
      const headRow = document.createElement('tr');
      for (const value of header) {
        const cell = document.createElement('th');
        cell.scope = 'col';
        appendInline(cell, value, onLocalFile);
        headRow.append(cell);
      }
      head.append(headRow); table.append(head);
      const body = document.createElement('tbody');
      i += 1;
      while (i + 1 < lines.length) {
        const row = tableCells(lines[i + 1]);
        if (!row || row.length !== header.length) break;
        i += 1;
        const tr = document.createElement('tr');
        for (const value of row) {
          const cell = document.createElement('td');
          appendInline(cell, value, onLocalFile);
          tr.append(cell);
        }
        body.append(tr);
      }
      table.append(body); scroll.append(table); fragment.append(scroll);
      continue;
    }
    if (/^ {0,3}> ?/.test(line)) {
      flushParagraph(); closeList(); formatted = true;
      const quote = document.createElement('blockquote');
      const content = [];
      do { content.push(lines[i].replace(/^ {0,3}> ?/, '')); i += 1; }
      while (i < lines.length && /^ {0,3}> ?/.test(lines[i]));
      i -= 1;
      const paragraph = document.createElement('p');
      appendInline(paragraph, content.join('\n'), onLocalFile);
      quote.append(paragraph); fragment.append(quote);
      continue;
    }
    if (/^ {0,3}(?:-{3,}|\*{3,}|_{3,})[ \t]*$/.test(line)) {
      flushParagraph(); closeList(); formatted = true;
      fragment.append(document.createElement('hr'));
      continue;
    }
    const heading = line.match(/^ {0,3}(#{1,3})[ \t]+(.+)$/);
    if (heading) {
      flushParagraph(); closeList(); formatted = true;
      const depth = heading[1].length;
      firstHeadingDepth ??= depth;
      const intendedLevel = Math.min(5, Math.max(3, 3 + depth - firstHeadingDepth));
      const level = Math.min(intendedLevel, previousHeadingLevel + 1);
      previousHeadingLevel = level;
      const node = document.createElement(`h${level}`);
      appendInline(node, heading[2], onLocalFile);
      fragment.append(node);
      continue;
    }
    const bullet = line.match(/^ {0,3}([-*+]|\d+\.)[ \t]+(.+)$/);
    if (bullet) {
      flushParagraph(); formatted = true;
      const kind = /\d/.test(bullet[1][0]) ? 'ol' : 'ul';
      if (!list || list.tagName.toLowerCase() !== kind) {
        list = document.createElement(kind);
        fragment.append(list);
      }
      const item = document.createElement('li');
      appendInline(item, bullet[2], onLocalFile);
      list.append(item);
      continue;
    }
    closeList();
    paragraph.push(line);
  }
  flushParagraph();
  target.replaceChildren(fragment);
  return formatted;
}
