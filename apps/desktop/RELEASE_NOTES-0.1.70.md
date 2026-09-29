# Forge Desktop 0.1.70 alpha — local candidate

Depois de escrever uma ideia, um clique em **Enviar e criar projeto** prepara
automaticamente a pasta padrão e envia aquela ideia quando o projeto está
pronto. Se você escolheu uma pasta, **Enviar e abrir projeto** faz o mesmo
nesse local. Quem prefere preparar o projeto antes de enviar ainda pode usar
os botões da área **Seu projeto**; esses botões não enviam mensagens.

Se a criação falhar, o rascunho é preservado e não ocorre envio automático
posterior. Se a pessoa mudar o texto ou sair da tela durante a preparação,
o envio pendente é cancelado. Uma conversa anterior que não puder ser retomada
também não recebe automaticamente a nova mensagem. Esta versão mantém a
correção da barra de navegação da 0.1.69 e o indicador de atividade da 0.1.68.

## Verificação e limites

Testes focados com navegador simulado confirmaram o envio único e a
preservação do rascunho nas falhas. No instalador instalado sobre a 0.1.69,
um único clique em **Enviar e abrir projeto** numa pasta de teste preparou o
projeto, enviou a ideia uma vez ao Codex real, produziu uma página local e
permitiu abrir a prévia e preparar um ajuste sem novo envio. A resposta levou
253 segundos; isso não é uma promessa de velocidade constante. O caminho
**Enviar e criar projeto** com pasta padrão foi confirmado por teste de UI
simulada, não por segundo envio real nesta rodada. Esta
alpha é Windows x64, sem assinatura e com atualização manual. A suíte ampla
de navegador tem uma expectativa de rótulo antiga não relacionada a este
fluxo. Não há download público desta versão até publicação e conferência.
