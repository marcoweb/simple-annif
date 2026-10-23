# Validação final em Windows limpo

Executar quando houver uma máquina ou VM Windows 10/11 x64 sem Python,
Rust, pip, Annif, uv ou ferramentas MSVC. A máquina de build não substitui
esse ambiente. Usar uma cópia descartável e dados de teste.

## Preparação

1. Copiar `dist/AnnifManager-win64.zip` e seu arquivo `.sha256`.
2. Comparar `Get-FileHash .\AnnifManager-win64.zip -Algorithm SHA256` com
   o conteúdo de `AnnifManager-win64.zip.sha256`.
3. Descompactar em uma pasta com espaços e acentos, como
   `C:\Teste Annif\Validação portátil\`.
4. Registrar edição/build do Windows, arquitetura, hash do ZIP, data e
   ausência das ferramentas de desenvolvimento. Desconectar a rede.

## Execução

- [ ] Abrir `AnnifManager.exe` pelo Explorer: janela sem console e sem erro de DLL.
- [ ] Diagnóstico: runtime incorporado, Python 3.13.16, Annif 1.4.2,
      logs legíveis e janela responsiva durante a execução.
- [ ] Executar a verificação abaixo e guardar o JSON e a saída do terminal.

Na pasta que contém `AnnifManager/`:

```powershell
.\AnnifManager\runtime\python.exe -I -B -u -X utf8 `
  .\AnnifManager\verify_runtime.py `
  .\AnnifManager\runtime `
  .\verification-windows-limpo.json
```

- [ ] Mover a pasta inteira e repetir diagnóstico e verificação a partir de
      outro diretório de trabalho. Confirmar caminhos no novo local.
- [ ] Colocar configurações TOML de teste em `workspace/projects/` e validar
      na UI IDs, nomes e backends da lista de projetos.
- [ ] Pela UI, carregar vocabulário UTF-8, treinar TF-IDF e MLLM e executar
      suggest nesses backends e no Ensemble. Usar nomes com espaços/acentos;
      conferir descritores, scores e logs. Registrar entradas e resultados.
- [ ] Trocar o workspace em Diagnóstico por outro caminho absoluto: novos
      dados devem ir para ele. Confirmar rejeição de caminho dentro de `runtime/`.
- [ ] Na cópia descartável, fechar a aplicação, renomear `runtime/` e abrir
      novamente: deve haver erro compreensível, sem fallback ou instalação.
      Restaurar a pasta; repetir retirando temporariamente `python.exe`.
- [ ] Em dados descartáveis, cancelar treinamento e carga de vocabulário:
      confirmar mensagem, logs preservados, ausência de processos Python do
      pacote em segundo plano e liberação das ações; repetir a operação até sucesso.
- [ ] Cancelar um lote de Suggest: preservar resultados anteriores e indicar os
      documentos cancelados/não processados no JSON; rejeitar CSV incompleto.
      Iniciar outra execução e confirmar conclusão normal. Trocar de aba durante
      o encerramento e verificar que a aplicação continua responsiva.

- [ ] Conferir tempo decorrido em carga, treinamento e Suggest, inclusive após
      trocar de aba. O valor deve parar ao terminar, falhar ou cancelar e zerar
      na próxima execução. Carga/treinamento mostram etapa e tempo restante
      indisponível; Suggest com vários documentos mostra contagem, barra e
      estimativa após o primeiro sucesso. Cancelados não contam como processados.

## Resultado

Guardar o JSON, hash do ZIP, capturas da UI e logs junto ao registro do ambiente.
Marcar cada item como aprovado, reprovado ou não executado, com detalhes das
falhas. A verificação CLI não comprova os fluxos da UI. Somente considerar a
validação portátil concluída quando todos os itens passarem neste ambiente.
