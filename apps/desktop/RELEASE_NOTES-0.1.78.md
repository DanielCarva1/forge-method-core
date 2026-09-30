# Forge Desktop 0.1.78 alpha

- Abrir uma pasta no Windows não deve mais falhar só porque ela contém um
  atalho de diretório (junction) para outra pasta.
- O Forge reconhece o atalho, sem entrar na pasta apontada. Os arquivos de
  fora não aparecem como resultados do projeto e não são alterados.
- A preparação e a consulta do projeto usam o mesmo caminho do motor,
  incluindo retomada, objetivo e registro cooperativo. Core incluído: 0.13.4.

## Limitações

Alpha Windows sem assinatura; atualização instalando o novo pacote.
Esta mudança é para o uso cooperativo comum, não autoriza seguir atalhos ou
ignorar erros. As operações protegidas de alteração/promoção e comprovação
formal de conclusão mantêm a validação rígida e podem rejeitar esses projetos.
Não inclui acesso remoto pelo celular, autenticação completa em máquina limpa
nem verificação com telefone físico/leitor de tela. Não declara o produto
completo. O teste nativo desta mudança usa servidor Codex simulado, não um
modelo real; a seleção gráfica de pasta não foi repetida nesta rodada.
