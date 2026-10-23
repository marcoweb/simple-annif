"""Verifica o runtime real, sem instalar pacotes ou acessar a rede."""
import importlib.metadata
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile


ENTRY_POINT = """import os, socket, nltk

def offline(*args, **kwargs):
    raise RuntimeError('Rede desabilitada na verificação do pacote')

socket.socket.connect = offline
socket.socket.connect_ex = offline
socket.create_connection = offline
nltk.data.path = [os.environ['NLTK_DATA']]
from annif.cli import cli
cli()
"""


def verify(runtime):
    runtime = runtime.resolve()
    if Path(sys.executable).resolve().parent != runtime:
        raise RuntimeError("A verificação deve usar o Python do pacote")
    if struct.calcsize("P") != 8 or not sys.flags.isolated:
        raise RuntimeError("É necessário Python x64 em modo isolado")
    for entry in sys.path:
        if not Path(entry).resolve().is_relative_to(runtime):
            raise RuntimeError(f"Caminho de módulos fora do pacote: {entry}")

    import numpy
    import sklearn
    import nltk
    import annif.backend

    nltk.data.path = [str(runtime / "nltk_data")]
    nltk.data.find("tokenizers/punkt_tab/english/")
    nltk.data.find("tokenizers/punkt_tab/portuguese/")
    nltk.data.find("corpora/stopwords/")
    for backend in ["tfidf", "mllm", "omikuji", "ensemble"]:
        annif.backend.get_backend(backend)

    report = {
        "python": sys.version.split()[0],
        "annif": importlib.metadata.version("annif"),
        "omikuji": importlib.metadata.version("omikuji"),
        "numpy": numpy.__version__,
        "scikit_learn": sklearn.__version__,
        "architecture": "x64",
        "network": "blocked_in_cli_tests",
        "commands": [],
    }
    with tempfile.TemporaryDirectory(prefix="Annif diagnóstico ") as temporary:
        workspace = Path(temporary)
        projects = workspace / "projects"
        projects.mkdir()
        (projects / "smoke.toml").write_text('''[smoke-tfidf]
name = "Prova TF-IDF"
backend = "tfidf"
language = "pt"
analyzer = "snowball(portuguese)"
vocab = "smoke"

[smoke-mllm]
name = "Prova MLLM"
backend = "mllm"
language = "pt"
analyzer = "snowball(portuguese)"
vocab = "smoke"
min_samples_leaf = 1

[smoke-omikuji]
name = "Prova Omikuji"
backend = "omikuji"
language = "pt"
analyzer = "snowball(portuguese)"
vocab = "smoke"

[smoke-ensemble]
name = "Prova Ensemble"
backend = "ensemble"
language = "pt"
vocab = "smoke"
sources = "smoke-tfidf:1.0,smoke-mllm:1.0,smoke-omikuji:1.0"
''', encoding="utf-8")
        computing = "https://example.org/computacao"
        gardening = "https://example.org/jardinagem"
        vocab = workspace / "vocabulário.tsv"
        vocab.write_text(f"{computing}\tComputação\n{gardening}\tJardinagem\n", encoding="utf-8")
        dataset = workspace / "dataset de treino"
        dataset.mkdir()
        for subject, text in [
            (computing, "Computação programação computadores software sistemas tecnologia. Referência secundária: jardinagem."),
            (gardening, "Jardinagem plantas flores jardins árvores cultivo natureza. Referência secundária: computação."),
        ]:
            for number in range(10):
                path = dataset / f"{subject.rsplit('/', 1)[1]} {number}.txt"
                path.write_text(text, encoding="utf-8")
                path.with_suffix(".key").write_text(f"<{subject}>\n", encoding="utf-8")
        document = workspace / "texto para indexação.txt"
        document.write_text("Computação programação computadores software sistemas.", encoding="utf-8")
        env = {key: value for key, value in os.environ.items()
               if not key.upper().startswith(("ANNIF_", "FLASK_"))}
        env.update(ANNIF_PROJECTS=str(projects), ANNIF_DATADIR=str(workspace / "models"),
                   ANNIF_CONFIG="annif.default_config.Config", NLTK_DATA=str(runtime / "nltk_data"),
                   HF_HUB_OFFLINE="1")

        def cli(*args):
            print(f"Verificando Annif: {args[0]} {' '.join(args[1:2])}", flush=True)
            result = subprocess.run(
                [sys.executable, "-I", "-B", "-u", "-X", "utf8", "-c", ENTRY_POINT, *args],
                cwd=workspace, env=env, capture_output=True, text=True,
                encoding="utf-8", errors="replace", timeout=180,
            )
            if result.returncode:
                raise RuntimeError(f"Annif {' '.join(args)} falhou:\n{result.stdout}\n{result.stderr}")
            report["commands"].append({"command": args[0], "arguments": list(args[1:2]), "status": "passed"})
            return result.stdout

        cli("--version")
        listing = cli("list-projects")
        if not all(name in listing for name in ["smoke-tfidf", "smoke-mllm", "smoke-omikuji", "smoke-ensemble"]):
            raise RuntimeError("A CLI não encontrou os projetos do workspace")
        cli("load-vocab", "smoke", str(vocab), "--language", "pt")
        for backend in ["tfidf", "mllm", "omikuji"]:
            cli("train", f"smoke-{backend}", str(dataset), "--jobs", "1")
        for backend in ["tfidf", "mllm", "omikuji", "ensemble"]:
            suggestions = cli("suggest", f"smoke-{backend}", str(document))
            if computing not in suggestions:
                raise RuntimeError(f"O backend {backend} não retornou o descritor esperado:\n{suggestions}")
            if "Computação" not in suggestions:
                raise RuntimeError("A CLI não preservou Unicode na saída")
    return report


if __name__ == "__main__":
    try:
        if len(sys.argv) != 3:
            raise RuntimeError("Uso: python -I -B -X utf8 verify_runtime.py RUNTIME RELATORIO.json")
        report = verify(Path(sys.argv[1]))
        Path(sys.argv[2]).write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        print(json.dumps(report, ensure_ascii=False))
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"Erro na verificação: {error}", file=sys.stderr)
        sys.exit(1)
