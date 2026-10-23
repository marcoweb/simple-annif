# Annif Manager

Aplicação desktop em Rust com egui/eframe para usar o Annif. Desenvolvimento
inicial no Linux; distribuição final para Windows 10/11 x64.

## Distribuição portátil

O usuário final deve apenas baixar o ZIP, descompactar e executar a aplicação.
O pacote deve conter tudo que ela precisa para funcionar, sem instalar Rust,
Python, pip, Annif ou ambientes virtuais e sem baixar dependências na primeira
execução.

```text
AnnifManager/
├── AnnifManager.exe
├── runtime/
│   ├── python.exe
│   ├── python3.dll
│   ├── python3XX.dll
│   ├── python3XX.zip
│   ├── python._pth
│   ├── Lib/site-packages/     # Annif e todas as dependências compatíveis
│   └── ...                   # DLLs e outros arquivos do Python incorporável
└── workspace/
    ├── projects/
    ├── vocabularies/
    ├── datasets/
    ├── models/
    ├── results/
    └── logs/
```

`3XX` representa a versão de Python escolhida para o pacote; os nomes reais
precisam corresponder à distribuição oficial dessa versão.

O `EmbeddedWindowsRuntime` localiza `runtime` a partir do executável da aplicação,
independentemente do diretório de trabalho. Usa exclusivamente o Python do
pacote para todos os comandos Annif. O modo isolado (`-I`) ignora variáveis
`PYTHON*` e pacotes do usuário; `-B` evita gravar bytecode no runtime e `-u`
permite saída sem buffering. `-X utf8` preserva acentos nos logs do Windows.
A aplicação define os caminhos de configuração e dados do Annif no workspace,
remove configurações `ANNIF_*` herdadas e aponta `NLTK_DATA` para os recursos
linguísticos incluídos no runtime.

A distribuição Windows exige um único arquivo `python._pth`, com caminhos
locais existentes, incluindo o diretório `site-packages` e `import site`.
Renomeie o arquivo `python3XX._pth` original para `python._pth`; não deixe os
dois arquivos no pacote. Usar o nome do executável evita depender de um nome
de DLL para selecionar a configuração. Caminhos absolutos, `..` e links para
fora do runtime são rejeitados. Isso evita que a busca de módulos dependa de
outra instalação registrada no Windows.

O ponto de entrada usado é `from annif.cli import cli; cli()`, o mesmo declarado
pelo [Annif](https://github.com/NatLibFi/Annif/blob/main/pyproject.toml).
Não depende de um launcher `annif.exe` ou de suporte a `python -m annif`.
As regras de isolamento seguem a documentação do
[Python incorporável](https://docs.python.org/3/using/windows.html#the-embeddable-package)
e do [modo isolado](https://docs.python.org/3/using/cmdline.html#cmdoption-I).

## Estado atual

O runtime incorporado está implementado e conectado às telas existentes.
A aplicação abre em **Diagnóstico**, mostrando o runtime selecionado. A
verificação mostra stdout, stderr, código de saída e duração, em segundo plano.
Código zero significa sucesso, inclusive quando a versão aparece em stderr.

A seleção ocorre na inicialização:

- Com `runtime` ao lado do executável, usa o runtime incorporado.
- Sem `runtime`, builds de desenvolvimento (`cargo run`) usam o `SystemRuntime`.
- Builds de distribuição (`--release`) sempre exigem o runtime incorporado.
- Um pacote ausente ou incompleto mostra um erro e nunca recorre ao Python do
  sistema no modo portátil.

O workspace padrão fica ao lado do executável e seus diretórios são preparados
antes de executar operações. Na tela de Diagnóstico, o workspace pode ser
alterado para outro caminho absoluto, fora do runtime. Essa configuração vale
para a sessão. Se a pasta padrão não for gravável, é possível selecionar outra.
As operações já iniciadas conservam o workspace que tinham ao começar.

Logs progressivos de treinamento e carga de vocabulário são capturados no
resultado final, além de enviados à UI. Uma saída diferente de zero é tratada
como falha nessas operações. Treinamento, carga de vocabulário e Suggest têm
**Cancelar processamento**, com preservação de logs. O cancelamento aguarda o
encerramento do processo e seus auxiliares. Carga/treinamento interrompidos podem
deixar arquivos parciais e devem ser refeitos antes de usar os projetos. Em Suggest,
o JSON mantém os resultados concluídos e registra documentos cancelados/não
processados; CSV para Evametrics rejeita lotes incompletos. Ainda não há timeout.

Os fluxos existentes foram corrigidos e verificados com o Annif incorporado:

- Projetos: formulário Novo projeto com nome, ID sugerido, idioma, vocabulário
  e método; fontes compatíveis para Ensemble; editor TOML avançado e listagem com
  nomes e backends reais obtidos por `show-project`.
- Vocabulários: idioma para TSV, suporte do Annif a RDF/SKOS e rejeição clara
  de páginas HTML salvas como arquivos de vocabulário.
- Treinamento: seleção de arquivo ou pasta, logs progressivos e orientação
  para Ensemble, que utiliza os modelos dos projetos fonte.
- Suggest: seleção de arquivos ou pastas TXT, resultados por documento com
  URI, assunto e score, falhas explícitas, exportação JSON estruturada e
  CSV para Evametrics (`ID;Titulo;Descriptores`). IDs vêm dos nomes dos arquivos
  e devem coincidir com o padrão-ouro; veja as restrições no guia de uso.
- Troca de workspace: limpa seleções e resultados anteriores; operações
  concorrentes que alteram o contexto são bloqueadas durante o processamento.

O pacote inclui `GUIA-USO.md` e `examples/` com dados didáticos e configuração
NALT em inglês. Consulte o [guia](packaging/GUIA-USO.md) para executar o fluxo
completo. Os fluxos principais passaram pela UI nativa na máquina de build:
carga TSV/RDF, treinamento TF-IDF/MLLM, Suggest e exportação pelo diálogo Windows.
Veja [o relatório da interface](packaging/validation-ui-windows.json).

O **ZIP completo Windows x64** foi compilado em release em 22 de outubro de
2026, com Python 3.13.16, Annif 1.4.2, dependências e dados linguísticos.
Treinamento, carga e Suggest mostram tempo decorrido. Suggest em lote também
exibe progresso por documento e tempo restante estimado pela média dos sucessos
da execução. Treinamento/carga mostram etapas, com tempo restante indisponível
porque o Annif não fornece progresso total. Consulte o
[guia de uso](packaging/GUIA-USO.md#acompanhar-o-tempo-e-o-progresso).

Formatação, check, Clippy, 26 testes Rust e três testes Python passaram.
Cancelamento de processos e recuperação estão registrados em
[validation-cancellation.json](packaging/validation-cancellation.json).
Corrigida também a substituição de arquivos do Annif no Windows ao repetir
carga/treinamento, mantendo as versões fixadas.
O CSV gerado pela UI nativa passou pelo código de importação do Evametrics;
veja [a validação de compatibilidade](packaging/validation-evametrics.json).
O recebimento de resultados e logs continua ao trocar de aba, liberando
as ações ao terminar. Suggest e treinamento identificam o projeto do resultado.
Para preparar uma amostra real de agricultura/NALT, veja
[TESTE-CORPUS-NALT.md](packaging/TESTE-CORPUS-NALT.md).
Em Vocabulários, **Atualizar vocabulários** consulta os IDs configurados e
mostra o estado, a quantidade de assuntos e os idiomas da última consulta.
Um teste adicional de integração com o runtime real foi executado separadamente
e passou por projetos, vocabulário, treinamento, sugestões, exportação e workspace.
A verificação CLI nativa passou na máquina de build, inclusive após extrair
e mover o pacote entre caminhos com espaços e acentos: carga de vocabulário,
treinamento TF-IDF/MLLM e suggest TF-IDF/MLLM/Ensemble com conexões bloqueadas.
O [relatório Windows](packaging/validation-windows.json) registra o hash e
os limites dessa validação; o relatório Wine anterior permanece preservado.
Em 22 de outubro de 2026, o usuário forneceu um relatório com oito testes CLI
aprovados e confirmou funcionamento geral da UI em outra máquina sem Python
instalado. Versão do Windows, hash testado e itens de relocação/workspace/runtime
ausente não foram confirmados; a validação completa permanece pendente. Esse
relato antecede a inclusão do cancelamento, que exige novos testes nessa máquina.
Uma falha de abertura no driver AMD `atio6axx.dll` foi contornada restringindo
a inicialização gráfica Windows a DirectX 12. O ZIP corrigido passou no teste
de abertura e resposta da janela nesta máquina, sem configuração adicional.
O [roteiro de teste](packaging/TESTE-WINDOWS-LIMPO.md) orienta essa etapa.
A aplicação não instala nem baixa pacotes; downloads ocorrem somente na
ferramenta de build do desenvolvedor.

## Desenvolvimento

Com Rust instalado e uma sessão gráfica disponível:

```sh
cargo run --locked
```

No modo de desenvolvimento sem runtime incorporado, informe executáveis de um
ambiente existente, por exemplo:

```text
Python: /caminho/meu ambiente/bin/python
Annif:  /caminho/meu ambiente/bin/annif
```

Informe os caminhos sem aspas e sem argumentos. Espaços e Unicode são aceitos.
Os nomes padrão (`python3` no Linux, `python` no Windows e `annif`) usam o PATH
somente nesse modo. A configuração não é salva entre execuções. As verificações
de Python e Annif são independentes no `SystemRuntime`: escolha executáveis do
mesmo ambiente.

Para testar a descoberta do runtime incorporado no Fedora, o layout esperado é
`runtime/bin/python3`, ao lado do executável em `target/debug/`. Esse runtime
Linux deve ser preparado separadamente; copiar um virtualenv não comprova
portabilidade. Um pacote Windows real não pode ser executado nesse sistema.

## Preparar o pacote Windows

A montagem está implementada em [packaging/build_portable.py](packaging/build_portable.py).
Consulte [packaging/README.md](packaging/README.md) para os pré-requisitos,
verificação e atualização de dependências.

No ambiente de build Windows, gerar o pacote completo:

```powershell
python packaging/build_portable.py
```

No Fedora, gerar e testar apenas o runtime Windows usando Wine:

```sh
python3 packaging/build_portable.py --runtime-only --wine
```

Os artefatos ficam em `dist/`, acompanhados de SHA-256. O ZIP do runtime não
contém a aplicação desktop. O pacote completo precisa do build release Windows
x64 de `AnnifManager.exe`. As versões e hashes das dependências estão fixados
no diretório `packaging/`; essa ferramenta não altera o Python global.

## Organização

```text
src/
├── main.rs                    # inicialização da janela
├── app.rs                     # navegação e runtime compartilhado
├── annif/
│   ├── runtime.rs             # contrato e resultado do processo
│   ├── commands.rs            # captura de saída e logs progressivos
│   ├── system_runtime.rs      # executáveis externos no desenvolvimento
│   └── embedded_runtime.rs    # Python incluído no pacote
├── services/
│   ├── runtime.rs             # seleção do runtime e troca de workspace
│   ├── workspace.rs           # preparação dos diretórios modificáveis
│   └── diagnostics.rs         # verificação em segundo plano e eventos
└── ui/
    └── diagnostics.rs         # apresentação e configuração do ambiente
```

O diagnóstico segue `ui → services → annif`. Os comandos são montados com
`Path`/`PathBuf` e argumentos separados, sem shell. Não foram adicionadas
dependências. No Windows, processos de CLI são iniciados sem abrir consoles.

## Verificar

```sh
cargo fmt --check
cargo check --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 -m unittest discover -s packaging -p 'test_*.py'
```

Os testes cobrem runtime ausente/incompleto, ausência de fallback, isolamento
do comando, caminhos de módulos dentro do pacote, mudança de diretório do
pacote, workspace separado/configurável, stdout/stderr, logs e falha por código
de saída. Os testes com scripts de execução são exclusivos de Unix; os testes
de seleção e montagem de comandos também podem rodar no Windows.
