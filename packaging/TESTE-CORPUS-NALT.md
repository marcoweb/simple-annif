# Teste funcional com corpus de agricultura

Fonte: [annif-experiments](https://github.com/marcoweb/annif-experiments/tree/7b039e495c4618daae2c8600f3950084c15dfabe/agriculture-en),
commit `7b039e495c4618daae2c8600f3950084c15dfabe`.

O teste usa D1–D100 do treinamento e D8001–D8005 do primeiro corpus de teste.
É uma amostra funcional, sem medição de qualidade ou treinamento completo.
Em 8 de outubro de 2026, a integração passou em 379,97 s, com 15 resultados
exportados. Relatório: `packaging/validation-corpus-nalt.json`.
Os textos de entrada contêm somente título e resumo, sem palavras-chave ou
referências. A indexação é convertida para URIs pelos prefLabel/altLabel em
inglês do NALT, com correspondência exata ignorando maiúsculas. Termos sem
correspondência ou ambíguos são excluídos e registrados, sem aproximações.

## Preparar no ambiente de desenvolvimento

Com o RDF verdadeiro em `target/nalt-input/nalt.rdf`, execute:

```powershell
python packaging/prepare_nalt_test_corpus.py
```

O script baixa apenas dados do commit fixado e produz:

- `target/corpus-validation/train/`: 100 pares UTF-8 `.txt`/`.key`.
- `target/corpus-validation/test/`: cinco documentos reservados para Sugestões de descritores.
- `target/corpus-validation/preparation.json`: procedência, hashes e termos excluídos.

Os dados ficam em `target/`, fora do Git e do ZIP da aplicação. A preparação
requer rede na primeira execução; a aplicação não baixa o corpus.

## Executar integração dos serviços

Reutiliza uma cópia do vocabulário já carregado; todos os modelos de treinamento
são criados em diretório temporário e removidos ao terminar. Não modifica o
workspace usado como origem do vocabulário.

```powershell
$env:ANNIF_TEST_PACKAGE = (Resolve-Path 'target/Validação funcional final/AnnifManager').Path
$env:ANNIF_TEST_CORPUS = (Resolve-Path 'target/corpus-validation').Path
$env:ANNIF_TEST_NALT_MODELS = (Resolve-Path 'target/nalt-input/workspace-final/models/vocabs/nalt').Path
cargo test --locked real_nalt_corpus_with_bundled_annif -- --ignored --nocapture
```

Valida TF-IDF e MLLM, Sugestões de descritores dos dois modelos e Ensemble nos cinco textos,
URIs NALT, scores finitos e exportação JSON dos 15 resultados.

## Repetir pela interface

Para testar Omikuji 0.2.0 com a mesma amostra e as variáveis acima:

```powershell
cargo test --locked omikuji_nalt_with_bundled_annif -- --ignored --nocapture
```

Esse teste cria um projeto Omikuji em workspace temporário, cancela o primeiro
treinamento após iniciar a preparação, treina novamente e gera descritores para
os cinco textos reservados. Verifica URIs, scores e exportações. Não mede qualidade.

Em uma cópia de teste do pacote, use `examples/projects-nalt.toml`, carregue
o RDF com ID `nalt` e selecione a pasta `train` no treinamento de cada modelo.
Selecione `test` em Sugestões de descritores para cada um dos três projetos e exporte os JSONs.
Durante operações, troque de aba: ao terminar, as ações devem ser liberadas
sem precisar voltar à aba de origem. Isso ainda exige validação nativa;
o teste de serviços não substitui a interação com diálogos e controles.
