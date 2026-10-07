# Montagem do pacote Windows x64

Estas ferramentas são executadas pelo desenvolvedor. O usuário final recebe
somente o ZIP pronto, descompacta e abre `AnnifManager.exe`.

## Versões fixadas

- [Python 3.13.16 incorporável x64](https://www.python.org/downloads/release/python-31316/), com o SHA-256 oficial registrado em `python-win64.json`.
- Annif 1.4.2, com as dependências Windows x64 em `requirements-win64.lock`.
- Dados NLTK `punkt_tab` e `stopwords`, com URLs e hashes em `nltk-data.json`.

A versão de Python respeita o intervalo suportado pelo
[Annif publicado no PyPI](https://pypi.org/pypi/annif/1.4.2/json).
Não são instalados extras de PyTorch, Transformers, fastText, spaCy ou Omikuji.
A ferramenta registra as versões de todas as bibliotecas no manifesto.

## Gerar o pacote completo no Windows

O ambiente de build precisa de Python 3.11+, uv, Rust com o target
`x86_64-pc-windows-msvc`, Visual Studio Build Tools/Windows SDK e acesso à rede
para obter os arquivos fixados. As ferramentas de compilação não são necessárias
na máquina de destino.

Da raiz do repositório:

```powershell
python packaging/build_portable.py
```

A ferramenta compila em release, monta `runtime/`, inclui dados linguísticos,
prepara um workspace vazio, verifica o runtime real e gera:

```text
dist/AnnifManager-win64.zip
dist/AnnifManager-win64.zip.sha256
```

O ZIP contém uma pasta `AnnifManager/` com executável, runtime, workspace,
licenças disponíveis nos pacotes, manifesto, lock de dependências e relatório
`verification.json`. A verificação não deixa corpus ou modelos de teste no
workspace entregue ao usuário.

O build MSVC usa CRT estática, conforme `.cargo/config.toml`, para evitar exigir
instalação separada do Visual C++ Redistributable para o executável Rust.
Essa [configuração](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes)
ainda precisa de validação do binário nativo e de suas DLLs na máquina limpa.
O executável da aplicação usa o subsistema gráfico no release Windows.
As DLLs da distribuição Python são preservadas no diretório `runtime`.

Também é possível fornecer um executável **release Windows x64** já compilado:

```powershell
python packaging/build_portable.py --exe caminho/annif_manager.exe
```

A ferramenta verifica o formato PE e a arquitetura x64. Quem fornece o
executável precisa confirmar que se trata de um build release, com a ligação
estática do CRT; esse modo não recompila nem inspeciona todas as DLLs do aplicativo.

## Preparar e validar o runtime no Fedora

```sh
python3 packaging/build_portable.py --runtime-only --wine
```

Isso usa wheels Windows, nunca bibliotecas Linux, e produz:

```text
dist/AnnifManager-runtime-win64.zip
dist/AnnifManager-runtime-win64.zip.sha256
```

**Esse ZIP contém apenas o runtime e os arquivos de apoio. Não contém
`AnnifManager.exe` e não é o aplicativo desktop pronto para o usuário final.**

Com um executável Windows previamente compilado, o Fedora também pode montar
o pacote completo:

```sh
python3 packaging/build_portable.py --exe caminho/annif_manager.exe --wine
```

Wine e winepath são necessários somente para a verificação no Linux.
A ferramenta usa um prefixo Wine próprio no cache do projeto.
`--runtime-only` sem `--wine` permite montar o runtime no Linux, mas o manifesto
registra que a verificação não foi executada.

Os downloads conferem hashes e os wheels são instalados com `--require-hashes`
e `--only-binary`. Downloads Python/NLTK ficam em `target/packaging-cache/`;
o uv usa também seu cache de wheels. Diretórios temporários são removidos ao
terminar, inclusive em caso de falha. Um ZIP existente só é substituído depois
de concluir uma nova montagem. `--output-dir` e `--cache` permitem escolher
outros diretórios. Não há instalação de pacotes no Python global.

## Verificação real

`verify_runtime.py` roda com o Python do pacote em modo isolado e UTF-8. Ele
confere que `sys.path` permanece dentro do runtime e que NumPy, scikit-learn e
os três backends prioritários carregam. Em um workspace temporário com espaços
e Unicode, executa:

1. Versão do Annif e listagem de projetos.
2. Carga de vocabulário TSV com descritores em português.
3. Treinamento TF-IDF e MLLM com exemplos positivos e negativos.
4. Suggest TF-IDF, MLLM e Ensemble, exigindo o descritor esperado e saída Unicode.

As conexões de rede são bloqueadas dentro dos processos Python da CLI. Os dados
NLTK são buscados exclusivamente no runtime durante o teste. Isso verifica
operações reais sem downloads; não testa todos os idiomas, configurações de
projeto ou bibliotecas opcionais. Ensemble usa os modelos dos dois backends
já treinados e não é treinado separadamente.

O relatório pode ser regenerado no Windows após descompactar e mover o pacote:

```powershell
.\AnnifManager\runtime\python.exe -I -B -u -X utf8 `
  .\AnnifManager\verify_runtime.py `
  .\AnnifManager\runtime `
  .\AnnifManager\verification.json
```

A verificação pelo Wine não substitui os testes numa máquina Windows 10/11 x64
sem Python ou outras ferramentas instaladas. Ainda devem ser conferidos o
executável gráfico, integração entre telas e runtime, DLLs e mudança de diretório
do pacote completo. O ZIP do runtime desta etapa foi verificado pelo Wine.
O [relatório registrado](validation-wine.json) inclui também a verificação
após descompactar o ZIP em outra pasta com espaços e acentos, além do hash
do artefato efetivamente testado.

## Atualizar dependências

Alterações no conjunto fixado são uma tarefa de manutenção do projeto:

```sh
uv pip compile packaging/requirements.in \
  --python-version 3.13 \
  --python-platform x86_64-pc-windows-msvc \
  --only-binary :all: --generate-hashes \
  --output-file packaging/requirements-win64.lock
```

Confira a compatibilidade, atualize hashes Python/NLTK quando necessário e
repita a montagem e a verificação real. Fixar versões e hashes reproduz o
conjunto de dependências; os ZIPs não prometem identidade byte a byte entre
ambientes de build diferentes. O SHA-256 ao lado de cada artefato identifica
o ZIP efetivamente gerado.

## Testes das ferramentas

```sh
python3 -m unittest discover -s packaging -p 'test_*.py'
```

Esses testes verificam rejeição de cache corrompido, caminhos de ZIP que escapam
do runtime e executáveis com formato ou arquitetura incorretos. A integração
com Annif real é verificada pela rotina de montagem acima.
