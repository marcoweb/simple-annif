# Annif Manager — instruções para retomar no Windows

Estado registrado em 7 de outubro de 2026. A próxima tarefa é compilar e testar
o pacote completo em Windows 10/11 x64. Leia também `README.md`,
`packaging/README.md` e `packaging/validation-wine.json` antes de alterar o código.

## Objetivo e decisões que devem ser preservadas

O usuário final deve baixar um ZIP, descompactar e executar `AnnifManager.exe`,
sem instalar Rust, Python, pip, Annif, ambientes virtuais ou bibliotecas, e sem
baixar dependências na primeira execução. Ferramentas instaladas no ambiente
de build do desenvolvedor não são requisitos da máquina de destino.

- Interface em Rust com egui/eframe; não migrar para Tauri, Electron ou frontend web.
- Annif executa os algoritmos; não reimplementá-los em Rust.
- Manter integração de processos isolada da UI, com serviços entre as camadas.
- Processos devem executar sem bloquear a UI e mostrar stdout/stderr e erros.
- Usar caminhos e argumentos separados, sem shell ou concatenação manual de comandos.
- Preservar suporte a espaços, Unicode e diferenças entre Linux e Windows.
- Dados modificáveis ficam no workspace, fora de `runtime/`.
- Priorizar TF-IDF, MLLM e Ensemble; não acrescentar extras pesados nesta etapa.
- Fazer alterações incrementais e manter o projeto compilável. Evitar novas
  dependências, grandes refatorações e funcionalidades fora do teste em andamento.

## Estado implementado

- A aplicação abre na aba Diagnóstico.
- `src/annif/embedded_runtime.rs` implementa `EmbeddedWindowsRuntime`.
- `src/services/runtime.rs` seleciona o runtime em relação a `current_exe()`.
- Builds release exigem runtime incorporado. Um pacote ausente/incompleto
  nunca deve recorrer ao Python ou Annif do PATH.
- Builds debug sem `runtime/` usam `SystemRuntime` para desenvolvimento.
- Todas as telas existentes usam a mesma seleção de runtime.
- A CLI é chamada pelo Python do pacote, com `from annif.cli import cli; cli()`.
  Não substituir por `python -m annif` sem verificar que a versão oferece esse suporte.
- Os argumentos Python são `-I -B -u -X utf8`: isolamento, ausência de gravação
  de bytecode, logs sem buffering e saída UTF-8.
- `ANNIF_PROJECTS` e `ANNIF_DATADIR` apontam para `workspace/projects` e
  `workspace/models`. Configurações `ANNIF_*` herdadas são removidas.
- `NLTK_DATA` aponta para `runtime/nltk_data`; os dados linguísticos estão incluídos.
- O workspace é configurável em Diagnóstico e vale para a sessão. Operações
  já iniciadas conservam o workspace anterior. Não há persistência dessa preferência.
- Logs progressivos são preservados no resultado final. Erros de treinamento
  e carga de vocabulário por código de saída diferente de zero são apresentados.
- Não há timeout ou cancelamento de processos na aplicação.
- As telas de projetos, vocabulários, treinamento e suggest já existiam e
  foram preservadas. Seus fluxos ainda precisam de validação pela UI. O parser
  da listagem de projetos é provisório; não considerar a integração pronta
  apenas porque a CLI passou nos testes.

## Arquivos essenciais para levar ao Windows

Transferir o repositório completo, incluindo arquivos ocultos:

```text
.cargo/config.toml
.gitignore
AGENTS.md
Cargo.toml
Cargo.lock
README.md
src/
packaging/
```

`target/` não precisa ser transferido: seus binários são Linux. `dist/` é
opcional e contém artefatos gerados, não versionados. Os fontes, locks,
instruções e scripts de empacotamento devem estar versionados para a retomada
por clone no Windows. Artefatos devem ser regenerados ou transferidos separadamente.
O repositório de destino é privado e se chama `simple-annif`. Não versionar
runtime instalado, workspace com dados do usuário ou arquivos de credenciais.

## Versões e artefato já gerado

- Python incorporável Windows x64: **3.13.16**, com URL e SHA-256 oficial em
  `packaging/python-win64.json`.
- Annif: **1.4.2**.
- NumPy: **2.2.6**; scikit-learn: **1.7.2**.
- As 62 dependências estão fixadas com hashes em `packaging/requirements-win64.lock`.
- Dados NLTK `punkt_tab` e `stopwords` têm hashes em `packaging/nltk-data.json`.

Foi gerado no Fedora:

```text
dist/AnnifManager-runtime-win64.zip
dist/AnnifManager-runtime-win64.zip.sha256
```

Esse ZIP tem aproximadamente 157 MiB e **não contém `AnnifManager.exe`**.
Seu SHA-256, também registrado em `packaging/validation-wine.json`, é:

```text
384e446884ec0bec6d19e3000ffd406a8a97a4af0eee641e739d15c86f5e99a5
```

O runtime real passou no Wine por carga de vocabulário, treinamento TF-IDF/MLLM
com exemplos positivos e negativos e suggest TF-IDF/MLLM/Ensemble. O teste
bloqueou conexões de rede nos processos Python da CLI e restringiu os dados NLTK
ao pacote. Também passou após descompactar em outra pasta com espaços e acentos.
Isso não equivale a uma validação nativa do aplicativo gráfico no Windows.

## Preparar o ambiente de build Windows

Usar Windows x64 com:

- Rust e target `x86_64-pc-windows-msvc`.
- Visual Studio Build Tools com ferramentas C++ e Windows SDK. Dependências
  nativas do Cargo podem exigir ferramentas adicionais de build; resolver
  a necessidade real a partir dos erros, sem alterar o contrato do pacote.
- Python **3.11 ou superior** para os scripts de build e testes das ferramentas.
- `uv` disponível no PATH do ambiente de build.
- Acesso à rede para os downloads fixados.

Executar da raiz do repositório em PowerShell, preferencialmente no terminal
com o ambiente de ferramentas MSVC configurado:

```powershell
rustup target add x86_64-pc-windows-msvc
cargo fmt --check
cargo check --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python -m unittest discover -s packaging -p 'test_*.py'
```

No Fedora passaram 9 testes Rust e 3 das ferramentas Python. Alguns testes
Rust são exclusivos de Unix; no Windows, um número menor é esperado. Não usar
a contagem total como único critério de sucesso.

## Gerar o pacote completo

```powershell
python packaging/build_portable.py
```

O script compila com `cargo build --release --locked --target
x86_64-pc-windows-msvc`, prepara o Python incorporável e as dependências Windows,
inclui dados linguísticos e licenças disponíveis, verifica o runtime real e gera:

```text
dist/AnnifManager-win64.zip
dist/AnnifManager-win64.zip.sha256
```

- `.cargo/config.toml` configura CRT estática para MSVC. Preservar isso para
  evitar exigir instalação separada do Visual C++ Redistributable para Rust.
  A ligação efetiva e as DLLs ainda precisam ser verificadas no binário nativo.
- `src/main.rs` configura o subsistema gráfico no release Windows, sem janela
  de console da aplicação. Processos filhos de CLI também devem ficar sem consoles.
- Os downloads conferem hashes; wheels usam `--require-hashes` e `--only-binary`.
- O script não instala pacotes no Python global do desenvolvedor.
- Python/NLTK ficam em cache em `target/packaging-cache`; uv usa seu próprio cache.
- Não alterar versões ou regenerar locks apenas para contornar uma falha de
  compilação. Primeiro identificar a causa e preservar a reprodutibilidade.

Alternativa com executável **release Windows x64** já compilado:

```powershell
python packaging/build_portable.py --exe caminho/annif_manager.exe
```

Esse modo verifica formato PE e arquitetura, mas não comprova build release
nem todas as dependências de DLL. Quem fornece o executável precisa verificá-los.

## Estrutura esperada após descompactar

```text
AnnifManager/
├── AnnifManager.exe
├── runtime/
│   ├── python.exe
│   ├── python313.dll
│   ├── python313.zip
│   ├── python._pth
│   ├── Lib/site-packages/
│   ├── nltk_data/
│   └── ... DLLs e demais arquivos do Python incorporável
├── workspace/
│   ├── projects/
│   ├── vocabularies/
│   ├── datasets/
│   ├── models/
│   ├── results/
│   └── logs/
├── licenses/
├── manifest.json
├── verification.json
└── verify_runtime.py
```

`runtime/python._pth` deve conter:

```text
python313.zip
.
Lib/site-packages
import site
```

O empacotador renomeia o arquivo original `python313._pth`. Não deixar ambos:
o runtime exige um único `python._pth` com caminhos locais existentes.
Não usar um virtualenv copiado como substituto do Python incorporável.

## Testar o pacote no Windows

Separar a máquina de build de uma máquina ou VM Windows 10/11 x64 limpa,
sem Python, Rust, pip, Annif, uv ou ferramentas MSVC instalados.

1. Conferir o SHA-256 do ZIP com o arquivo `.sha256`:

   ```powershell
   Get-FileHash .\dist\AnnifManager-win64.zip -Algorithm SHA256
   ```

2. Descompactar numa pasta com espaços e acentos. Iniciar `AnnifManager.exe`
   pelo Explorer e confirmar que a janela abre sem console e sem erros de DLL.
3. Em Diagnóstico, confirmar runtime incorporado, Python 3.13.16, Annif 1.4.2,
   stdout/stderr legíveis e UI responsiva durante a verificação.
4. Mover o diretório inteiro para outro local. Iniciar o executável a partir
   de outro diretório de trabalho e repetir o diagnóstico. Os caminhos devem
   acompanhar o pacote, sem usar arquivos da instalação anterior.
5. Repetir diagnóstico e operações sem acesso à rede. Não devem ocorrer
   downloads de bibliotecas, dados NLTK ou outros recursos dos backends prioritários.
6. Reexecutar a verificação real com o Python incluído, a partir da pasta que
   contém `AnnifManager/`:

   ```powershell
   .\AnnifManager\runtime\python.exe -I -B -u -X utf8 `
     .\AnnifManager\verify_runtime.py `
     .\AnnifManager\runtime `
     .\AnnifManager\verification-windows.json
   ```

   Esse teste usa workspace temporário e não apaga os dados do workspace do usuário.
7. Testar pela UI: listagem e seleção de projetos, carga de vocabulário,
   treinamento e suggest com arquivos UTF-8 e nomes com espaços/acentos.
   Colocar os projetos TOML em `workspace/projects/`. Conferir nomes, IDs,
   backends apresentados, scores e logs. Investigar diferenças do parser da
   CLI; não confundir saída textual capturada com resultado estruturado.
8. Alterar o workspace em Diagnóstico para um caminho absoluto fora de
   `runtime/`. Verificar a criação dos diretórios e que novos dados/modelos
   sejam gravados nele. Confirmar que uma seleção dentro de `runtime/` é rejeitada.
9. Em uma **cópia descartável do pacote**, renomear `runtime/`, retirar
   `python.exe` ou um arquivo necessário e iniciar a aplicação release.
   Deve aparecer erro compreensível, sem fallback ou tentativa de instalar Python.
   Restaurar a cópia ao terminar; não modificar o pacote com dados do usuário.

## Registrar resultados e continuar

Registrar versão/arquitetura do Windows, hash do ZIP, ambiente de teste,
verificações executadas, stdout/stderr relevantes e relatório JSON. Distinguir
resultado nativo Windows de resultado Wine; não substituir o relatório Wine
por um resultado Windows sem identificar a origem.

Não declarar o aplicativo portátil completo antes de testar o executável
release e os fluxos da UI em Windows limpo. Corrigir primeiro falhas dessa base.
Ao modificar Rust, executar `cargo fmt`, `cargo check --locked` e testes
apropriados; executar Clippy quando aplicável. Mudanças no empacotador exigem
seus testes e uma nova montagem/verificação real quando afetarem o runtime.
Não avançar para histórico, SQLite, avaliações ou novas telas enquanto essa
validação estiver pendente.
