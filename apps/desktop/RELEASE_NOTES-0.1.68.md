# Forge Desktop 0.1.68 alpha — local candidate

O app informa, em linguagem simples, quando o Codex está conferindo o projeto,
alterando arquivos ou preparando a resposta. Não mostra porcentagem inventada
nem expõe o texto privado dos comandos. Uma atividade desconhecida continua
com a indicação genérica de trabalho.

O instalador também corrige a descoberta do diretório de build do Core quando
o Cargo usa um cache configurado fora da pasta do projeto.

## Verificação e limites

O candidato foi instalado sobre a 0.1.67. No executável instalado, um projeto
novo recebeu uma única mensagem real: o agente criou uma página local, o app
mostrou atividade, abriu a página na prévia e preparou um pedido de mudança sem
enviá-lo. A resposta levou 186 segundos. Isso não prova desempenho constante,
nem cobertura de toda a UI. Testes focados do Desktop passaram. A suíte ampla
de navegador parou antes dos novos cenários por esperar o nome antigo de um
botão de atualização; este teste precisa ser atualizado.

Esta alpha é Windows x64, sem assinatura e com atualização manual. A prévia
local não publica nem executa o produto como aplicação distribuída. O download
da 0.1.68 não está disponível publicamente; publicação exige aprovação do
conteúdo e conferência do arquivo baixado.
