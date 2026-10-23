# Usar o Annif Manager

## Criar um projeto pelo formulário

1. Em **Projetos → Novo projeto**, informe um nome. O identificador é sugerido
   automaticamente e pode ser alterado.
2. Escolha o idioma dos documentos e informe o ID do vocabulário, como `nalt`.
   Se já houver vocabulários configurados, escolha um na lista de existentes.
3. Escolha **TF-IDF**, **MLLM** ou **Omikuji** e clique em **Criar projeto**. A aplicação salva
   um novo arquivo no workspace, atualiza a lista e seleciona o projeto criado.
4. Em **Vocabulários**, carregue o arquivo correspondente ao ID informado.
   Depois vá à aba **Treinamento** e, ao concluir, à aba **Sugestões de descritores**.

Para criar um **Ensemble**, escolha pelo menos dois projetos TF-IDF/MLLM/Omikuji com o
mesmo idioma e vocabulário. Os pesos começam iguais. Treine as fontes antes
de gerar sugestões de descritores; não é necessário treinar o Ensemble separadamente.

O formulário não sobrescreve projetos existentes. Identificadores repetidos
são rejeitados. Outros idiomas, parâmetros e pesos podem ser configurados em
**Avançado: editar TOML**. Workspaces antigos com arquivos `.cfg` precisam ser
convertidos para TOML antes de usar o formulário.

## Experimentar com os exemplos incluídos

1. Extraia todo o ZIP e abra `AnnifManager.exe`. Em Diagnóstico, clique em
   **Verificar ambiente**. Python e Annif devem apresentar código zero.
2. Em Projetos → **Avançado: editar TOML**, clique em **Inserir exemplo
   TF-IDF / MLLM / Ensemble** e depois **Salvar Alterações**. No pacote,
   o caminho inicial é `workspace/projects/projects.toml`. O exemplo só pode
   ser inserido quando o editor está vazio, para preservar suas alterações.
3. Volte a **Lista de Projetos**, clique em **Atualizar Lista via Annif** e
   confirme os três projetos. A validação do editor verifica sintaxe e campos
   essenciais; a lista via Annif verifica se a configuração pode ser carregada.
4. Em Vocabulários, clique em **Atualizar vocabulários**, selecione `meu-vocab`, informe idioma `pt` e selecione
   `examples/vocabulario.tsv`. Clique em **Carregar Vocabulário** e aguarde
   a mensagem de sucesso. O vocabulário precisa estar referenciado no TOML.
5. Em Projetos, selecione `meu-tfidf`. Em Treinamento, use **Selecionar arquivo**
   para abrir `examples/treinamento.tsv`, inicie e aguarde a conclusão.
   Repita para `meu-mllm`. Ensemble utiliza os modelos dos projetos fonte;
   não é necessário treiná-lo separadamente.
6. Selecione um dos três projetos. Em Sugestões de descritores, selecione
   `examples/documento.txt` e clique em **Sugerir descritores**. Os resultados
   apresentam descritor, score e URI. A opção **Saída completa** contém os logs.
7. Use **Exportar resultados (JSON)** para salvar os resultados estruturados,
   com arquivo de origem, projeto, sugestões, scores, duração e eventuais erros.
   Uma falha em um documento não impede o processamento dos próximos.

Os exemplos são pequenos dados didáticos, não modelos prontos para uso real.
Substitua projetos, vocabulário e corpus pelos seus dados para uso produtivo.

## Usar Omikuji (versão 0.2.0)

Em **Projetos → Novo projeto**, escolha **Omikuji**. Informe o idioma e o
vocabulário dos documentos. Carregue o vocabulário, treine com textos associados
às URIs dos descritores e use **Sugerir descritores**. O backend já está incluído
no pacote Windows e não exige instalação nem download na primeira execução.

O formulário usa os padrões do Annif para Omikuji (Parabel). Para experimentar
com os exemplos acima, crie um projeto Omikuji em português com vocabulário
`meu-vocab` e treine com `examples/treinamento.tsv`. O arquivo
`examples/projects-omikuji.toml` também oferece um projeto Omikuji e um Ensemble
que o combina com `meu-tfidf`; copie-o para `workspace/projects/` apenas se esses
IDs ainda não existirem. Treine ambas as fontes antes de usar esse Ensemble.

Para NALT em inglês, use vocabulário `nalt` e idioma `en`. O arquivo
`examples/projects-nalt-omikuji.toml` contém essa configuração; ele complementa
`projects-nalt.toml`. O RDF é o vocabulário, não o corpus de treinamento.

Corpora grandes podem exigir bastante memória e tempo. O treinamento usa os
núcleos disponíveis; parâmetros como `min_df`, `ngram`, `cluster_k` e `max_depth`
podem ser ajustados no editor TOML avançado, conforme a
[documentação do Annif](https://github.com/NatLibFi/Annif/wiki/Backend%3A-Omikuji).
Cancelamento, logs, tempo decorrido e exportações funcionam como nos demais
métodos. O tempo restante de treinamento continua indisponível. Depois de
cancelar, repita o treinamento até concluir antes de usar o modelo.

Ao atualizar de uma versão anterior, use o **pacote completo**: trocar apenas
o EXE não instala o Omikuji no runtime antigo. Preserve seu workspace e mantenha
uma cópia da instalação anterior. Modelos existentes TF-IDF/MLLM continuam
separados; um projeto Omikuji precisa de seu próprio treinamento.

## Formatos

### Exportar para Evametrics

Após gerar sugestões de descritores, clique em **Exportar para Evametrics (CSV)**. O arquivo
UTF-8 usa `ID;Titulo;Descriptores`, conforme o importador de
[marcoweb/evametrics](https://github.com/marcoweb/evametrics/blob/9b2b9dc3d8d28bb53722ad9de63349d33598077b/components/dataset-load-control.tsx).
Importe-o como **Dataset**, junto ao seu padrão-ouro no Evametrics.

- O ID vem do nome do arquivo sem extensão. `D8001.txt` gera `D8001`;
  `8001.txt` também gera `D8001`. Outros nomes recebem o prefixo `D` se necessário.
  Os IDs devem coincidir exatamente com os do padrão-ouro; não há renumeração.
- A coluna de título usa o nome do arquivo sem extensão. Os descritores são os
  nomes dos assuntos retornados pelo Annif, separados por vírgula. Scores, URIs
  e logs permanecem disponíveis na exportação JSON.
- Descritores repetidos, ignorando maiúsculas, são exportados uma única vez.
- O importador atual não representa corretamente listas vazias, nem descritores
  com vírgulas, ponto e vírgula, aspas ou quebras de linha. Nesses casos, a
  exportação mostra um erro e mantém o destino intacto. JSON preserva esses dados.
- Resultados com falhas, IDs repetidos ou projetos diferentes no mesmo arquivo
  são rejeitados para evitar omissões e comparações incorretas.

Exemplo:

```csv
ID;Titulo;Descriptores
D8001;D8001;"corn, flowering, soil"
```

### Arquivos de entrada

- Todos os arquivos de texto devem ser UTF-8.
- Vocabulário TSV: uma linha por assunto, `URI<TAB>descritor`. Informe seu
  idioma (por exemplo `pt`). SKOS/RDF e CSV também são aceitos pelo Annif.
- Corpus TSV: `texto<TAB>URI1 URI2`. Não use tabulações ou quebras de linha
  dentro do texto de uma linha. Cada documento precisa de assuntos conhecidos.
- Corpus em pasta: pares `documento.txt` e `documento.key`; o `.key` contém
  os assuntos do documento, uma URI por linha (por exemplo `<https://ex/1>`).
- Sugestões de descritores: arquivos de texto. Ao selecionar uma pasta, somente seus arquivos
  `.txt` são usados, sem subpastas. Arquivos repetidos são processados uma vez.

## Workspace e operações

O workspace contém configurações, modelos e dados modificáveis, separados do
runtime. A troca em Diagnóstico exige caminho absoluto fora de `runtime/`;
vale somente nesta sessão. Após a troca, selecione novamente o projeto.
Durante operações, aguarde a conclusão antes de mudar o ambiente ou iniciar
outra operação. A janela continua responsiva e os logs se atualizam.

Para abrir um TOML existente, use Procurar ou Carregar arquivo indicado.
Para criar outro arquivo, edite o caminho ou use Salvar como para escolher o
novo destino e salvar. No modo portátil, salve dentro de
`projects/` do workspace ativo. Arquivos TOML inválidos não substituem o arquivo
existente. Após salvar, atualize a lista antes de treinar ou solicitar sugestões.

Mensagens de erro incluem o retorno do Annif. Uma saída em stderr sozinha não
significa falha; a execução precisa terminar com código zero. Ainda não há timeout.

### Acompanhar o tempo e o progresso

Treinamento, carga de vocabulário e Sugestões de descritores mostram o **tempo decorrido**.
O cronômetro continua ao trocar de aba e para ao concluir, falhar ou cancelar;
uma nova execução começa do zero.

Em **Sugestões de descritores**, a barra mostra os documentos processados e o total do lote,
com contadores separados para falhas e cancelamentos. Depois do primeiro
documento concluído com sucesso, aparece o **tempo restante estimado**, calculado
pela média dos documentos bem-sucedidos desta execução. Textos de tamanhos
diferentes podem alterar bastante essa previsão. Quando o documento atual
excede a média, a interface informa que está recalculando. Não há estimativa
durante o cancelamento nem depois de encerrar a operação.

Treinamento e carga de vocabulário mostram a etapa informada pelo Annif.
Como essas operações não informam progresso total, o tempo restante aparece
como indisponível. Não há previsão baseada em execuções anteriores.

### Cancelar uma operação

Durante **Treinamento**, **Carregar Vocabulário** ou **Sugestões de descritores**, clique em
**Cancelar processamento** na aba da operação. Aguarde a mensagem de cancelamento;
as ações só serão liberadas após encerrar o processo. Os logs recebidos são mantidos.

- Treinamento e carga são interrompidos sem desfazer arquivos já gravados pelo
  Annif. O modelo ou vocabulário pode ficar incompleto: repita a operação até
  concluir com sucesso antes de usar os projetos afetados.
- Em Sugestões de descritores, os resultados concluídos são mantidos. O documento interrompido
  e os restantes recebem indicação de cancelamento ou de não processamento.
  Exporte JSON para guardar o lote completo, incluindo `cancelled` e `error`.
  O CSV para Evametrics rejeita esse lote incompleto; execute novamente antes
  de exportar para avaliação.
- Uma operação que já terminou conserva seu resultado, mesmo se o pedido de
  cancelamento chegar logo depois. Para tentar novamente, inicie uma nova operação.
- Consultas de projetos, vocabulários e Diagnóstico ainda não têm cancelamento.

Prefira cancelar e aguardar a confirmação antes de fechar a aplicação.

A validação final em uma máquina Windows limpa segue pendente. Consulte o
roteiro `TESTE-WINDOWS-LIMPO.md` fornecido junto ao pacote.


## NALT em RDF/SKOS

O exemplo `examples/projects-nalt.toml` define três projetos NALT em inglês.
Copie esse TOML para `workspace/projects/`, atualize a lista e carregue o RDF
com ID `nalt`. Para RDF/SKOS, os idiomas vêm do próprio arquivo; no campo
Idioma pode manter `en` para este exemplo.

Baixe o arquivo bruto, não a página HTML do GitHub:
[Download do NALT RDF](https://github.com/marcoweb/annif-experiments/raw/refs/heads/main/agriculture-en/raw-files/vocabulary/nalt-full_dwn_20240716.rdf).
Um arquivo salvo a partir da página de visualização pode ter extensão `.rdf`
e conter HTML; a aplicação identifica esse caso e orienta baixar o arquivo bruto.
O RDF não é incluído no ZIP do aplicativo.

O vocabulário fornece os assuntos e seus descritores. Para treinar TF-IDF e
MLLM, ainda é necessário um corpus de textos em inglês associado às URIs do
NALT. Os exemplos de computação/jardinagem deste pacote têm outro vocabulário
e não devem ser usados para treinar os projetos NALT.
