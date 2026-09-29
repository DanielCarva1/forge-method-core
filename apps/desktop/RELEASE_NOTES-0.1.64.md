# Forge Desktop 0.1.64 alpha.1

**Comece um projeto sem escolher pasta ou nome.** Escreva sua ideia e clique em
**Começar meu projeto**. O Forge cria uma pasta nova em Documentos → Projetos
Forge, sugere o nome a partir da ideia e inicia o projeto. Se o nome já existir,
cria outro sem sobrescrever arquivos. A ideia continua como rascunho e não é
enviada automaticamente. Escolher outra pasta no diálogo do Windows ou abrir
uma pasta existente continua disponível.

Também é possível continuar escrevendo um rascunho enquanto o Codex trabalha
ou está desconectado. **Enviar** continua bloqueado até a conversa estar pronta;
reabrir o app não reenvia uma mensagem incerta.

## Verificação e limites

O fluxo de nova pasta passou no app Windows nativo instalado sobre 0.1.63:
criação real em Documentos, inicialização pelo Forge, rascunho preservado e
nenhuma mensagem enviada ao Codex. O fluxo de escolher outra pasta também
passou no app instalado, com o seletor real do Windows. Testes focados cobriram
erro e início único do projeto. O crate Desktop
passou seus testes Rust; um teste de associação padrão do navegador permanece
ignorado por depender da configuração local.
Os testes de rascunho usam um Codex controlado; desconexão real e uma nova
resposta real do Codex não foram exercitadas neste pacote. A suíte ampla e o
CI manual do GitHub não foram executados.

Continua sendo uma alpha Windows x64 sem assinatura, com atualização manual
por instalador NSIS. O app usa o Forge Core 0.13.2 incluído no pacote.
Instalação em máquina limpa, primeiro login completo, leitor de tela manual e
publicação de projetos na internet permanecem fora desta verificação.
