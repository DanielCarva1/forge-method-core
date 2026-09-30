// A read-only view of actual project files. Native validation owns every open.
const kinds = { page: 'Página', image: 'Imagem', document: 'Documento', data: 'Dados', audio: 'Áudio', video: 'Vídeo', archive: 'Pacote' };
export function createResultFiles(openFile) {
  const list = document.getElementById('result-files-list');
  const status = document.getElementById('result-files-status');
  const search = document.getElementById('result-files-search');
  const type = document.getElementById('result-files-type');
  const refresh = document.getElementById('refresh-result-files');
  let root = null;
  let generation = 0;
  let files = [];
  let selected = null;
  let truncated = false;
  let pending = false;
  let refreshAgain = false;
  function paint() {
    const query = search.value.trim().toLocaleLowerCase('pt-BR');
    const visible = files.filter(file => (!type.value || file.kind === type.value) && file.relative_path.toLocaleLowerCase('pt-BR').includes(query));
    list.replaceChildren(...visible.map(file => {
      const button = document.createElement('button');
      button.type = 'button'; button.className = 'result-file-card';
      button.dataset.resultPath = file.relative_path;
      button.setAttribute('aria-label', `Ver ${kinds[file.kind].toLowerCase()}: ${file.relative_path}`);
      button.setAttribute('aria-pressed', String(selected === file.relative_path));
      const badge = document.createElement('span'); badge.className = 'result-file-kind'; badge.textContent = kinds[file.kind];
      const title = document.createElement('strong'); title.textContent = file.relative_path.split('/').at(-1);
      const path = document.createElement('small'); path.textContent = file.relative_path;
      const size = document.createElement('small'); size.className = 'result-file-size';
      size.textContent = file.size_bytes >= 1048576 ? `${(file.size_bytes / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.ceil(file.size_bytes / 1024))} KB`;
      button.append(badge, title, path, size);
      button.addEventListener('click', () => { void openFile(file.relative_path); });
      return button;
    }));
    status.textContent = pending ? 'Procurando arquivos nesta pasta…'
      : !files.length ? 'Ainda não encontramos páginas, imagens ou documentos nesta pasta. Você pode pedir ao agente para criar algo ou escolher um arquivo.'
      : !visible.length ? 'Nenhum arquivo combina com essa busca. Tente outro nome ou tipo.'
      : `${visible.length} ${visible.length === 1 ? 'arquivo para conferir' : 'arquivos para conferir'}.${truncated ? ' A lista é parcial; use Escolher arquivo para encontrar outros.' : ''}`;
    refresh.disabled = !root || pending;
  }
  async function load() {
    if (!root) return;
    if (pending) { refreshAgain = true; return; }
    const current = generation;
    const projectRoot = root;
    pending = true; paint();
    try {
      const page = await globalThis.__TAURI__.core.invoke('list_project_files', { projectRoot });
      if (current !== generation) return;
      if (!page || !Array.isArray(page.files) || page.files.length > 100 || typeof page.truncated !== 'boolean'
        || page.files.some(file => !file || !kinds[file.kind] || typeof file.relative_path !== 'string' || !file.relative_path || file.relative_path.length > 1024
          || !Number.isSafeInteger(file.size_bytes) || file.size_bytes < 0)) throw new Error('Invalid file list');
      files = page.files; truncated = page.truncated; pending = false; paint();
    } catch {
      if (current === generation) { pending = false; paint(); status.textContent = 'Não foi possível atualizar a lista. A lista anterior foi mantida; você ainda pode escolher um arquivo.'; }
    } finally {
      if (current === generation && refreshAgain) { refreshAgain = false; void load(); }
    }
  }
  search.addEventListener('input', paint); type.addEventListener('change', paint);
  refresh.addEventListener('click', () => { void load(); });
  return {
    setProject(project) { generation++; root = project?.project_root ?? null; files = []; selected = null; pending = false; refreshAgain = false; truncated = false; search.value = ''; type.value = ''; paint(); if (root) void load(); },
    select(path) { selected = path.replace(/\\/g, '/'); paint(); },
    refresh: load,
  };
}
