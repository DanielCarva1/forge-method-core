# Forge Desktop 0.1.65 alpha

Agora é possível começar um projeto diretamente pela tela inicial ou por
**Meus projetos**. Ao escrever uma ideia, o botão da conversa prepara
automaticamente uma pasta nova em Documentos → Projetos Forge. A ideia fica como
rascunho: o app não a envia ao Codex até você clicar em **Enviar** depois que o
projeto estiver pronto. A mesma criação direta funciona ao escolher um tema em
**Explorar**. Se preferir uma pasta existente ou personalizada, essa opção
continua disponível.

Em **Onde estamos**, o objetivo e o próximo passo registrados aparecem antes
dos detalhes de etapa do Forge, que podem ser abertos sob demanda.

## Verificação e limites

Testes focados em navegador e no app Windows nativo cobriram criação real de
projeto, escolha de pasta pelo diálogo do Windows, rascunho não enviado e a
apresentação do registro. Os testes de navegador usam Codex simulado; não houve
um novo envio e resposta reais do Codex nesta versão. A suíte ampla de
navegador contém expectativas antigas e não foi aprovada para este pacote.
O instalador foi aplicado sobre a versão 0.1.64 e os dois fluxos nativos
principais passaram no executável 0.1.65 instalado. Ainda não houve publicação
da versão 0.1.65 para download.

Esta é uma alpha Windows x64 sem assinatura, com atualização manual por
instalador NSIS. O pacote continua usando o Forge Core 0.13.2 incluído no app;
as alterações locais do Core 0.13.3 não fazem parte dele. Instalação em máquina
limpa, login inicial completo e leitor de tela manual não foram verificados.
