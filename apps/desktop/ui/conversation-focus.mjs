const workspace = document.querySelector('.workspace');
const expand = document.getElementById('conversation-focus');
const showContext = document.getElementById('conversation-show-context');
const options = document.getElementById('conversation-options');

export function setConversationFocus(focused) {
  workspace.classList.toggle('conversation-focus', focused);
  showContext.hidden = !focused;
  expand.hidden = focused;
  if (focused) options.open = false;
}

expand.addEventListener('click', () => setConversationFocus(true));
showContext.addEventListener('click', () => setConversationFocus(false));
