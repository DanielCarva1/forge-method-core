// The selected path is transient UI state. Forge owns the project, not this preview.
import { renderAgentMessage } from './message-format.mjs';
import { setMobileWorkspaceProject, showWorkspacePane } from './mobile-workspace.mjs';
const panel = document.getElementById('project-preview');
const heading = document.getElementById('preview-heading');
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
const fileNote = document.getElementById('preview-file-note');
const markdown = document.getElementById('preview-markdown');
const openPreview = document.getElementById('open-preview');
const browserAction = document.getElementById('preview-browser-action');
const previewOrigin = document.querySelector('#preview-result .preview-origin');
const openSiteBrowser = document.getElementById('open-site-browser');
const browserLabel = document.getElementById('open-browser-label');
const browserHint = document.getElementById('open-browser-hint');
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
let previewReadPending = false;
let browserOpening = false;
let renderUrl = null;
let formattedMarkdown = false;
let fileOnly = false;
let pdfFile = false;
let sourceVisible = false;
let dialogSiteHeight = 560;
let refreshAfterDialog = false;
let refreshAfterPendingRead = false;
let refreshAfterPicker = false;

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
  fileOnly = false;
  pdfFile = false;
  sourceVisible = false;
  result.hidden = true;
  openPreview.hidden = true;
  browserAction.hidden = true;
  previewOrigin.after(browserAction);
  requestChange.classList.add('primary');
  openSiteBrowser.classList.remove('primary');
  browserLabel.textContent = 'Usar no navegador';
  browserHint.textContent = 'Fora da prévia protegida, a página pode executar código e acessar a internet. Abra apenas projetos de confiança.';
  fileNote.textContent = 'Este arquivo está na pasta do projeto, mas não pode ser mostrado aqui. Peça ao agente para explicar o resultado ou diga o que gostaria de mudar.';
  sourceToggle.hidden = true;
  image.hidden = true;
  image.removeAttribute('src');
  image.alt = '';
  text.hidden = true;
  text.textContent = '';
  fileNote.hidden = true;
  heading.textContent = 'Prévia do resultado';
  refresh.textContent = 'Atualizar prévia';
  requestChange.textContent = 'Pedir mudança neste arquivo';
  markdown.replaceChildren();
  markdown.hidden = true;
  site.removeAttribute('src');
  site.hidden = true;
  siteNote.hidden = true;
  siteNote.open = false;
  pathLabel.textContent = '';
}

export function setPreviewProject(value) {
  if (project && (!value || value.project_root !== project.project_root)) {
    try { void globalThis.__TAURI__?.core?.invoke('clear_preview_site').catch(() => {}); } catch { /* UI remains safe if native cleanup fails. */ }
  }
  project = value;
  // Keep the result entry point ahead of the record in both visual and
  // keyboard order, including before any file has been selected.
  if (value) {
    workspace.insertBefore(panel, recordPanel);
    if (workspace.lastElementChild !== projectPanel) workspace.append(projectPanel);
  }
  if (!value && workspace.firstElementChild !== projectPanel) workspace.prepend(projectPanel);
  filePath = null;
  generation++;
  pending = false;
  previewReadPending = false;
  refreshAfterDialog = false;
  refreshAfterPendingRead = false;
  refreshAfterPicker = false;
  clearResult();
  panel.hidden = !value;
  workspace.classList.toggle('project-ready', !!value);
  workspace.classList.remove('preview-loaded');
  setMobileWorkspaceProject(value);
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
  previewReadPending = true;
  clearResult();
  controls();
  status.hidden = false;
  status.textContent = 'Conferindo o arquivo local…';
  try {
    const preview = await globalThis.__TAURI__.core.invoke('inspect_preview', { projectRoot: root, filePath: selected });
    if (current !== generation) return;
    if (!preview || !['image', 'text', 'file'].includes(preview.kind) || typeof preview.content !== 'string' || (preview.kind === 'file' && preview.content !== '') || typeof preview.relative_path !== 'string' || !preview.relative_path || !Number.isSafeInteger(preview.size_bytes) || preview.size_bytes < 0) throw new Error('Invalid preview');
    if (preview.render_url !== undefined && (preview.kind !== 'text' || !/^http:\/\/forgepreview\.localhost\/[a-f0-9]{32}\/[A-Za-z0-9%._~-]+$/.test(preview.render_url))) throw new Error('Invalid site preview');
    pathLabel.textContent = preview.relative_path;
    if (preview.kind === 'file') {
      fileOnly = true;
      pdfFile = /\.pdf$/i.test(preview.relative_path);
      fileNote.hidden = false;
      if (pdfFile) {
        fileNote.textContent = 'Este PDF não aparece na prévia protegida.';
        browserLabel.textContent = 'Abrir PDF no navegador';
        browserHint.textContent = 'Abre fora do Forge. Use apenas arquivos de projetos de confiança.';
        fileNote.after(browserAction);
        openSiteBrowser.classList.add('primary');
        requestChange.classList.remove('primary');
      }
      heading.textContent = pdfFile ? 'PDF do projeto' : 'Arquivo do projeto';
      refresh.textContent = 'Atualizar informações';
      requestChange.textContent = 'Conversar sobre este arquivo';
    } else if (preview.kind === 'image') {
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
    openPreview.hidden = fileOnly;
    browserAction.hidden = !renderUrl && !pdfFile;
    workspace.classList.add('preview-loaded');
    status.textContent = fileOnly ? 'Arquivo encontrado na pasta do projeto.' : 'Prévia local atualizada.';
  } catch (error) {
    if (current === generation) {
      workspace.classList.remove('preview-loaded');
      status.textContent = typeof error === 'string' ? error : 'Não foi possível mostrar este arquivo. Ele não foi alterado; escolha outro ou tente atualizar.';
    }
  } finally {
    if (current === generation) {
      pending = false;
      previewReadPending = false;
      controls();
      // A terminal event may have arrived while this read was in flight.
      // Re-read only a successful selected preview; never retry a failed one.
      if (refreshAfterPendingRead) {
        refreshAfterPendingRead = false;
        if (!result.hidden) void loadPreview();
      }
    }
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
    clearResult();
    document.getElementById('preview-heading').focus({ preventScroll: true });
  } catch {
    if (current === generation) status.textContent = 'Não foi possível escolher um arquivo. Tente novamente.';
    return;
  } finally {
    if (current === generation) {
      pending = false;
      controls();
      if (refreshAfterPicker) {
        refreshAfterPicker = false;
        // A canceled picker preserves the prior file. A newly selected file
        // already receives its own read below.
        if (!result.hidden) void loadPreview();
      }
    }
  }
  if (current === generation) void loadPreview();
});
refresh.addEventListener('click', loadPreview);
export async function refreshPreviewAfterTurn() {
  // A stopped Codex turn may already have changed the selected file. Reuse the
  // native project-bound read; never infer another file from reply text.
  if (!project || !filePath) return;
  if (previewReadPending) {
    refreshAfterPendingRead = true;
    return;
  }
  if (pending) {
    if (!result.hidden) refreshAfterPicker = true;
    return;
  }
  if (result.hidden) return;
  if (dialog.open) {
    refreshAfterDialog = true;
    dialogStatus.textContent = 'A conversa parou. A prévia será atualizada ao fechar esta janela.';
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
  controls();
  await loadPreview();
  // Keep both the result and any validation error visible to the reader.
  showWorkspacePane('preview', true);
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
  if (!project || !filePath || (!renderUrl && !pdfFile) || result.hidden || pending || browserOpening) return;
  const root = project.project_root;
  const selected = filePath;
  const openingPdf = pdfFile;
  browserOpening = true;
  controls();
  try {
    await globalThis.__TAURI__.core.invoke(openingPdf ? 'open_pdf_in_browser' : 'open_site_in_browser', { projectRoot: root, filePath: selected });
    if (project?.project_root === root && filePath === selected) status.textContent = openingPdf
      ? 'Abertura do PDF solicitada ao navegador padrão, fora da prévia protegida do Forge.'
      : 'Abertura solicitada ao navegador padrão. Esta página roda fora da prévia protegida do Forge.';
  } catch {
    if (project?.project_root === root && filePath === selected) status.textContent = openingPdf
      ? 'Não foi possível abrir este PDF no navegador. Confira o navegador padrão nas configurações do Windows; o arquivo não foi alterado.'
      : 'Não foi possível abrir esta página no navegador. Confira o navegador padrão nas configurações do Windows; o arquivo não foi alterado.';
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
addEventListener('hashchange', clearExpanded);

function prepareChangeRequest() {
  if (!project || result.hidden) return;
  if (composer.disabled) {
    const message = 'Aguarde a conversa ficar pronta antes de pedir uma mudança.';
    status.textContent = message;
    dialogStatus.textContent = message;
    return;
  }
  const request = fileOnly ? `Sobre o arquivo ${pathLabel.textContent}: ` : `Quero mudar o arquivo ${pathLabel.textContent}: `;
  composer.value = composer.value.trim() ? `${composer.value.trimEnd()}\n${request}` : request;
  if (dialog.open) dialog.close();
  showWorkspacePane('conversation');
  composer.focus();
  status.textContent = 'Pedido preparado na conversa. Revise e envie quando quiser.';
}
requestChange.addEventListener('click', prepareChangeRequest);
dialogRequestChange.addEventListener('click', prepareChangeRequest);
