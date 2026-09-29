# Forge Desktop 0.1.67 alpha

O app agora reúne a criação direta de projetos e o registro mais simples da
0.1.65, o Core 0.13.3 incluído na 0.1.66 e uma orientação mais leve para o
agente. Uma ideia escrita em **Início** ou escolhida em **Explorar** pode virar
um projeto em Documentos → Projetos Forge sem exigir que você crie uma pasta.
A ideia fica como rascunho até você clicar em **Enviar**. Escolher uma pasta
existente continua disponível.

Para trabalho local pequeno e reversível, o agente é orientado a fazer e
verificar o resultado sem criar registros de trabalho, encerrar etapas ou
admitir evidência só por formalidade. A continuidade e as proteções do Forge
continuam disponíveis quando há trabalho longo ou um efeito real a proteger.
Isso responde a um teste controlado no app 0.1.66: uma página simples foi
criada corretamente, mas levou 370 segundos e 29 chamadas de ferramenta,
incluindo vários registros que não eram necessários para aquele resultado.
No teste comparável no app 0.1.67, a página foi criada e conferida em 262
segundos, com 17 chamadas de ferramenta. Isso mostra melhora neste exemplo,
não garante a mesma economia em outros projetos.

## Verificação e limites

O instalador candidato foi aplicado sobre a 0.1.66; os hashes e a verificação
nativa estão no checkpoint do app. A criação de uma página real no projeto de
teste passou no executável 0.1.67 instalado. Ainda levou mais de quatro
minutos, portanto a experiência de espera precisa melhorar. Esta é uma alpha
Windows x64, sem assinatura e com atualização manual. O app não oferece todos
os tipos de interação nativa
do Codex; pedidos de interação não suportados não são aprovados
automaticamente. A prévia local não substitui executar ou publicar o produto
criado. Instalação em máquina limpa, leitor de tela manual e a suíte ampla de
navegador ainda não foram verificados. Só haverá download desta versão após
publicação e conferência do arquivo.
