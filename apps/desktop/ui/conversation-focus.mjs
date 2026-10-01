import { decorateControl } from './control-icons.mjs';
const workspace = document.querySelector('.workspace');
const expand = document.getElementById('conversation-focus');
const showContext = document.getElementById('conversation-show-context');
const options = document.getElementById('conversation-options');
const summary = options.querySelector('summary');
decorateControl(summary, 'more', 'Opções da conversa');
summary.classList.add('secondary-control');

// A small anchored menu, not another panel taking space from the conversation.
options.addEventListener('keydown', event => {
  if (event.key === 'Escape' && options.open) {
    event.preventDefault(); options.open = false; summary.focus();
  }
});
document.addEventListener('pointerdown', event => {
  if (options.open && !options.contains(event.target)) options.open = false;
});
options.addEventListener('focusout', event => {
  if (event.relatedTarget && !options.contains(event.relatedTarget)) options.open = false;
});
options.addEventListener('click', event => {
  if (event.target.closest('button')) {
    options.open = false;
    if (options.contains(document.activeElement)) summary.focus();
  }
});

export function setConversationFocus(focused) {
  workspace.classList.toggle('conversation-focus', focused);
  showContext.hidden = !focused;
  expand.hidden = focused;
  if (focused) options.open = false;
  workspace.dispatchEvent(new CustomEvent('forge:workspace-focus', { detail: focused }));
}

expand.addEventListener('click', () => setConversationFocus(true));
showContext.addEventListener('click', () => setConversationFocus(false));
