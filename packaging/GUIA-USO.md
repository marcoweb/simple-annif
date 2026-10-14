# Usar o Annif Manager

## Primeiro projeto

1. Extraia todo o ZIP e abra `AnnifManager.exe`. Em Diagnóstico, clique em
   **Verificar ambiente**. Python e Annif devem apresentar código zero.
2. Em Projetos → **Editar projects.toml**, clique em **Inserir exemplo
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
6. Selecione um dos três projetos. Em Suggest, selecione
   `examples/documento.txt` e clique em **Executar Suggest**. Os resultados
   apresentam assunto, score e URI. A opção **Saída completa** contém os logs.
7. Use **Exportar resultados (JSON)** para salvar os resultados estruturados,
   com arquivo de origem, projeto, sugestões, scores, duração e eventuais erros.
   Uma falha em um documento não impede o processamento dos próximos.

Os exemplos são pequenos dados didáticos, não modelos prontos para uso real.
Substitua projetos, vocabulário e corpus pelos seus dados para uso produtivo.

## Formatos

- Todos os arquivos de texto devem ser UTF-8.
- Vocabulário TSV: uma linha por assunto, `URI<TAB>descritor`. Informe seu
  idioma (por exemplo `pt`). SKOS/RDF e CSV também são aceitos pelo Annif.
- Corpus TSV: `texto<TAB>URI1 URI2`. Não use tabulações ou quebras de linha
  dentro do texto de uma linha. Cada documento precisa de assuntos conhecidos.
- Corpus em pasta: pares `documento.txt` e `documento.key`; o `.key` contém
  os assuntos do documento, uma URI por linha (por exemplo `<https://ex/1>`).
- Suggest: arquivos de texto. Ao selecionar uma pasta, somente seus arquivos
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
significa falha; a execução precisa terminar com código zero. Não há timeout
nem cancelamento: não feche o aplicativo durante treinamento ou carga.

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
