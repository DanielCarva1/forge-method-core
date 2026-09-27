// The selected path is transient UI state. Forge owns the project, not this preview.
import { renderAgentMessage } from './message-format.mjs';
const panel = document.getElementById('project-preview');
const workspace = document.querySelector('.workspace');
const projectPanel = workspace.querySelector('.project');
const recordPanel = workspace.querySelector('.record');
const choose = document.getElementById('choose-preview');
const refresh = document.getElementById('refresh-preview');
const status = document.getElementById('preview-status');
const result = document.getElementById('preview-result');
const pathLabel = document.getElementById('preview-path');
const image = document.getElementById('preview-image');
const site = document.getElementById('preview-site');
const siteNote = document.getElementById('preview-site-note');
const text = document.getElementById('preview-text');
const markdown = document.getElementById('preview-markdown');
const openPreview = document.getElementById('open-preview');
const browserAction = document.getElementById('preview-browser-action');
const openSiteBrowser = document.getElementById('open-site-browser');
const sourceToggle = document.getElementById('preview-source-toggle');
const dialog = document.getElementById('preview-dialog');
const closePreview = document.getElementById('close-preview');
const dialogPath = document.getElementById('preview-dialog-path');
const dialogImage = document.getElementById('preview-dialog-image');
const dialogSite = document.getElementById('preview-dialog-site');
const dialogMore = document.getElementById('preview-dialog-more');
const dialogText = document.getElementById('preview-dialog-text');
const dialogMarkdown = document.getElementById('preview-dialog-markdown');
const dialogSourceToggle = document.getElementById('preview-dialog-source-toggle');
const dialogStatus = document.getElementById('preview-dialog-status');
const dialogSiteNote = document.getElementById('preview-dialog-site-note');
const dialogRequestChange = document.getElementById('request-preview-change-large');
const requestChange = document.getElementById('request-preview-change');
const composer = document.getElementById('message-text');
let project = null;
let filePath = null;
let generation = 0;
let pending = false;
let browserOpening = false;
let renderUrl = null;
let formattedMarkdown = false;
let sourceVisible = false;
let dialogSiteHeight = 560;
let refreshAfterDialog = false;

function showSource(value) {
  sourceVisible = value;
  site.hidden = !renderUrl || sourceVisible;
  markdown.hidden = !formattedMarkdown || sourceVisible;
  text.hidden = (renderUrl || formattedMarkdown) && !sourceVisible;
  dialogSite.hidden = !renderUrl || sourceVisible;
  dialogMore.hidden = !renderUrl || sourceVisible || dialogSiteHeight >= 8000;
  dialogMarkdown.hidden = !formattedMarkdown || sourceVisible;
  dialogText.hidden = (renderUrl || formattedMarkdown) && !sourceVisible;
  sourceToggle.textContent = dialogSourceToggle.textContent = formattedMarkdown
    ? sourceVisible ? 'Ver leitura' : 'Ver texto original'
    : sourceVisible ? 'Ver prévia visual' : 'Ver código';
}

function controls() {
  choose.disabled = !project || pending;
  refresh.hidden = !filePath;
  refresh.disabled = !project || !filePath || pending;
  openPreview.disabled = !project || result.hidden || pending;
  openSiteBrowser.disabled = !project || !filePath || result.hidden || pending || browserOpening;
}

function clearExpanded() {
  if (dialog.open) dialog.close();
  dialogImage.removeAttribute('src');
  dialogImage.alt = '';
  dialogImage.hidden = true;
  dialogText.textContent = '';
  dialogText.hidden = true;
  dialogMarkdown.replaceChildren();
  dialogMarkdown.hidden = true;
  dialogSite.removeAttribute('src');
  dialogSite.hidden = true;
  dialogSite.style.removeProperty('height');
  dialogSiteHeight = 560;
  dialogMore.hidden = true;
  dialogSourceToggle.hidden = true;
  dialogPath.textContent = '';
  dialogStatus.textContent = '';
  dialogSiteNote.hidden = true;
}

function clearResult() {
  clearExpanded();
  renderUrl = null;
  formattedMarkdown = false;
  sourceVisible = false;
  result.hidden = true;
  openPreview.hidden = true;
  browserAction.hidden = true;
  sourceToggle.hidden = true;
  image.hidden = true;
  image.removeAttribute('src');
  image.alt = '';
  text.hidden = true;
  text.textContent = '';
  markdown.replaceChildren();
  markdown.hidden = true;
  site.removeAttribute('src');
  site.hidden = true;
  siteNote.hidden = true;
  siteNote.open = false;
  pathLabel.textContent = '';
}

function promotePreview() {
  if (workspace.classList.contains('preview-engaged')) return;
  const focused = panel.contains(document.activeElement) ? document.activeElement : null;
  workspace.insertBefore(panel, recordPanel);
  workspace.classList.add('preview-engaged');
  focused?.focus({ preventScroll: true });
}

export function setPreviewProject(value) {
  if (project && (!value || value.project_root !== project.project_root)) {
    try { void globalThis.__TAURI__?.core?.invoke('clear_preview_site').catch(() => {}); } catch { /* UI remains safe if native cleanup fails. */ }
  }
  project = value;
  // A real record precedes an empty preview. Once the person opens a file,
  // the result becomes primary; DOM and visual order change together.
  if (value) {
    workspace.insertBefore(recordPanel, panel);
    if (workspace.lastElementChild !== projectPanel) workspace.append(projectPanel);
  }
  if (!value && workspace.firstElementChild !== projectPanel) workspace.prepend(projectPanel);
  filePath = null;
  generation++;
  pending = false;
  refreshAfterDialog = false;
  clearResult();
  panel.hidden = !value;
  workspace.classList.toggle('project-ready', !!value);
  workspace.classList.remove('preview-loaded');
  workspace.classList.remove('preview-engaged');
  status.hidden = true;
  status.textContent = '';
  controls();
}

async function loadPreview() {
  if (!project || !filePath || pending) return;
  const current = ++generation;
  const root = project.project_root;
  const selected = filePath;
  pending = true;
  clearResult();
  controls();
  status.hidden = false;
  status.textContent = 'Conferindo o arquivo local…';
  try {
    const preview = await globalThis.__TAURI__.core.invoke('inspect_preview', { projectRoot: root, filePath: selected });
    if (current !== generation) return;
    if (!preview || !['image', 'text'].includes(preview.kind) || typeof preview.content !== 'string' || typeof preview.relative_path !== 'string' || !preview.relative_path || !Number.isSafeInteger(preview.size_bytes)) throw new Error('Invalid preview');
    if (preview.render_url !== undefined && (preview.kind !== 'text' || !/^http:\/\/forgepreview\.localhost\/[a-f0-9]{32}\/[A-Za-z0-9%._~-]+$/.test(preview.render_url))) throw new Error('Invalid site preview');
    pathLabel.textContent = preview.relative_path;
    if (preview.kind === 'image') {
      if (!/^data:image\/(png|jpeg|gif|webp);base64,[A-Za-z0-9+/=]+$/.test(preview.content)) throw new Error('Invalid image');
      image.src = preview.content;
      image.alt = `Prévia local de ${preview.relative_path}`;
      image.hidden = false;
    } else {
      text.textContent = preview.content;
      renderUrl = preview.render_url || null;
      if (renderUrl) {
        site.src = renderUrl;
        siteNote.hidden = false;
        sourceToggle.hidden = false;
        showSource(false);
      } else if (/\.md$/i.test(preview.relative_path)) {
        formattedMarkdown = true;
        renderAgentMessage(markdown, preview.content);
        sourceToggle.hidden = false;
        showSource(false);
      } else text.hidden = false;
    }
    result.hidden = false;
    openPreview.hidden = false;
    browserAction.hidden = !renderUrl;
    workspace.classList.add('preview-loaded');
    status.textContent = 'Prévia local atualizada.';
  } catch (error) {
    if (current === generation) {
      workspace.classList.remove('preview-loaded');
      status.textContent = typeof error === 'string' ? error : 'Não foi possível mostrar este arquivo. Ele não foi alterado; escolha outro ou tente atualizar.';
    }
  } finally {
    if (current === generation) { pending = false; controls(); }
  }
}

choose.addEventListener('click', async () => {
  if (!project || pending) return;
  const current = ++generation;
  pending = true;
  controls();
  status.hidden = false;
  status.textContent = 'Escolhendo um arquivo do projeto…';
  try {
    const selected = await globalThis.__TAURI__.core.invoke('choose_preview_file');
    if (current !== generation) return;
    if (!selected) {
      status.textContent = filePath ? 'Seleção cancelada. A prévia anterior não foi alterada.' : 'Seleção cancelada. Nenhum arquivo escolhido.';
      return;
    }
    if (typeof selected !== 'string') throw new Error('Invalid path');
    filePath = selected;
    promotePreview();
    clearResult();
    document.getElementById('preview-heading').focus({ preventScroll: true });
  } catch {
    if (current === generation) status.textContent = 'Não foi possível escolher um arquivo. Tente novamente.';
    return;
  } finally {
    if (current === generation) { pending = false; controls(); }
  }
  if (current === generation) void loadPreview();
});
refresh.addEventListener('click', loadPreview);
export async function refreshPreviewAfterTurn() {
  // A completed Codex turn may have changed the selected file. Reuse the
  // native project-bound read; never infer another file from reply text.
  if (!project || !filePath || result.hidden || pending) return;
  if (dialog.open) {
    refreshAfterDialog = true;
    dialogStatus.textContent = 'O Codex terminou. A prévia será atualizada ao fechar esta janela.';
    return;
  }
  await loadPreview();
}
export async function previewLinkedFile(candidate) {
  if (!project || pending || typeof candidate !== 'string' || candidate.length > 1024) return;
  const path = candidate.replace(/\//g, '\\');
  const rooted = /^[A-Za-z]:\\/.test(path)
    ? path
    : !/^(?:\\|[A-Za-z][A-Za-z0-9+.-]*:)/.test(path)
      ? `${project.project_root.replace(/[\\/]+$/, '')}\\${path}`
      : null;
  if (!rooted) return;
  filePath = rooted;
  promotePreview();
  controls();
  await loadPreview();
  // Keep both the result and any validation error visible to the reader.
  panel.scrollIntoView({ block: 'start' });
}
openPreview.addEventListener('click', () => {
  if (!project || result.hidden || pending || dialog.open) return;
  dialogPath.textContent = `Arquivo: ${pathLabel.textContent}`;
  dialogSiteNote.hidden = !renderUrl;
  if (!image.hidden) {
    dialogImage.src = image.src;
    dialogImage.alt = image.alt;
    dialogImage.hidden = false;
  } else if (renderUrl || formattedMarkdown) {
    if (renderUrl) dialogSite.src = renderUrl;
    dialogText.textContent = text.textContent;
    if (formattedMarkdown) renderAgentMessage(dialogMarkdown, text.textContent);
    dialogSourceToggle.hidden = false;
    showSource(sourceVisible);
  } else {
    dialogText.textContent = text.textContent;
    dialogText.hidden = false;
  }
  dialogRequestChange.disabled = composer.disabled;
  dialog.showModal();
});
openSiteBrowser.addEventListener('click', async () => {
  if (!project || !filePath || !renderUrl || result.hidden || pending || browserOpening) return;
  const root = project.project_root;
  const selected = filePath;
  browserOpening = true;
  controls();
  try {
    await globalThis.__TAURI__.core.invoke('open_site_in_browser', { projectRoot: root, filePath: selected });
    if (project?.project_root === root && filePath === selected) status.textContent = 'Abertura solicitada ao navegador padrão. Esta página roda fora da prévia protegida do Forge.';
  } catch {
    if (project?.project_root === root && filePath === selected) status.textContent = 'Não foi possível abrir esta página no navegador. O arquivo não foi alterado.';
  } finally { browserOpening = false; controls(); }
});
sourceToggle.addEventListener('click', () => showSource(!sourceVisible));
dialogSourceToggle.addEventListener('click', () => showSource(!sourceVisible));
dialogMore.addEventListener('click', () => {
  if (!dialog.open || !renderUrl || sourceVisible) return;
  dialogSiteHeight = Math.min(dialogSiteHeight + 800, 8000);
  dialogSite.style.height = `${dialogSiteHeight}px`;
  dialogMore.hidden = dialogSiteHeight >= 8000;
});
closePreview.addEventListener('click', () => dialog.close());
dialog.addEventListener('close', () => {
  clearExpanded();
  if (refreshAfterDialog) {
    refreshAfterDialog = false;
    void refreshPreviewAfterTurn();
  }
});
addEventListener('hashchange', () => { refreshAfterDialog = false; clearExpanded(); });

function prepareChangeRequest() {
  if (!project || result.hidden) return;
  if (composer.disabled) {
    const message = 'Aguarde a conversa ficar pronta antes de pedir uma mudança.';
    status.textContent = message;
    dialogStatus.textContent = message;
    return;
  }
  const request = `Quero mudar o arquivo ${pathLabel.textContent}: `;
  composer.value = composer.value.trim() ? `${composer.value.trimEnd()}\n${request}` : request;
  if (dialog.open) dialog.close();
  composer.focus();
  status.textContent = 'Pedido preparado na conversa. Revise e envie quando quiser.';
}
requestChange.addEventListener('click', prepareChangeRequest);
dialogRequestChange.addEventListener('click', prepareChangeRequest);
