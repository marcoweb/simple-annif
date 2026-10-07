"""Monta o pacote Windows x64. Ferramenta do desenvolvedor, não do usuário final."""
import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import struct
import subprocess
import sys
import tempfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / "packaging"
TARGET = "x86_64-pc-windows-msvc"
DIRECTORIES = ["projects", "vocabularies", "datasets", "models", "results", "logs"]


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def download(metadata, cache):
    path = cache / metadata["filename"]
    if not path.exists():
        temporary = path.with_suffix(".download")
        try:
            with urllib.request.urlopen(metadata["url"], timeout=60) as source, temporary.open("wb") as output:
                shutil.copyfileobj(source, output)
            if digest(temporary) != metadata["sha256"]:
                raise RuntimeError(f"Hash inválido para {metadata['filename']}")
            temporary.replace(path)
        finally:
            temporary.unlink(missing_ok=True)
    if digest(path) != metadata["sha256"]:
        raise RuntimeError(f"Arquivo em cache não corresponde ao hash: {path}")
    return path


def extract(archive, destination):
    with zipfile.ZipFile(archive) as source:
        for member in source.infolist():
            name = PurePosixPath(member.filename.replace("\\", "/"))
            if name.is_absolute() or ".." in name.parts or ":" in str(name):
                raise RuntimeError(f"Caminho inválido no ZIP: {member.filename}")
            if (member.external_attr >> 16) & 0o170000 == 0o120000:
                raise RuntimeError("Links simbólicos não são aceitos nos arquivos do runtime")
        source.extractall(destination)


def require_windows_executable(path):
    with path.open("rb") as source:
        header = source.read(64)
        if len(header) < 64 or header[:2] != b"MZ":
            raise RuntimeError("O executável fornecido não é um binário Windows")
        source.seek(struct.unpack_from("<I", header, 60)[0])
        signature = source.read(6)
        if signature != b"PE\0\0\x64\x86":
            raise RuntimeError("É necessário um executável Windows x64")


def verify(package, cache, use_wine):
    runtime = package / "runtime"
    report = package / "verification.json"
    script = CONFIG / "verify_runtime.py"
    env = os.environ.copy()
    if os.name == "nt":
        command = [str(runtime / "python.exe"), "-I", "-B", "-u", "-X", "utf8",
                   str(script), str(runtime), str(report)]
    elif use_wine:
        for tool in ["wine", "winepath"]:
            if not shutil.which(tool):
                raise RuntimeError(f"Ferramenta de validação não encontrada: {tool}")
        env.update(WINEPREFIX=str(cache / "wine"), WINEDEBUG="-all")

        def windows_path(path):
            return subprocess.check_output(["winepath", "-w", str(path)], env=env, text=True, timeout=60).strip()

        command = ["wine", str(runtime / "python.exe"), "-I", "-B", "-u", "-X", "utf8",
                   windows_path(script), windows_path(runtime), windows_path(report)]
    else:
        return {"status": "not_run", "reason": "Requires Windows or --wine"}
    subprocess.run(command, cwd=cache, env=env, check=True, timeout=600)
    return {"status": "passed", "environment": "windows" if os.name == "nt" else "wine",
            "report": "verification.json"}


def build(args):
    uv = shutil.which("uv")
    if not uv:
        raise RuntimeError("Instale uv no ambiente de build do desenvolvedor")
    executable = args.exe.resolve() if args.exe else None
    if not args.runtime_only:
        if executable is None:
            if os.name != "nt":
                raise RuntimeError("No Linux, use --runtime-only ou --exe com um build Windows x64")
            subprocess.run(["cargo", "build", "--release", "--locked", "--target", TARGET], cwd=ROOT, check=True)
            executable = ROOT / "target" / TARGET / "release" / "annif_manager.exe"
        require_windows_executable(executable)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    cache = args.cache.resolve()
    cache.mkdir(parents=True, exist_ok=True)
    python = json.loads((CONFIG / "python-win64.json").read_text(encoding="utf-8"))
    resources = json.loads((CONFIG / "nltk-data.json").read_text(encoding="utf-8"))
    lock = CONFIG / "requirements-win64.lock"
    with tempfile.TemporaryDirectory(prefix="annif-package-", dir=args.output_dir) as temporary:
        package = Path(temporary).resolve() / "AnnifManager"
        runtime = package / "runtime"
        runtime.mkdir(parents=True)
        extract(download(python, cache), runtime)
        packages = runtime / "Lib" / "site-packages"
        subprocess.run([
            uv, "pip", "install", "--target", str(packages), "--python-version", "3.13",
            "--python-platform", TARGET, "--only-binary", ":all:", "--require-hashes",
            "--no-deps", "--no-sources", "--index-url", "https://pypi.org/simple",
            "--requirements", str(lock),
        ], check=True)
        # Launchers gerados pelo instalador podem conter caminhos do ambiente de build.
        # A aplicação chama a CLI pelo Python incorporado; não distribui esses launchers.
        for directory in ["bin", "Scripts"]:
            shutil.rmtree(packages / directory, ignore_errors=True)
        for original in runtime.glob("python*._pth"):
            original.unlink()
        (runtime / "python._pth").write_text("python313.zip\n.\nLib/site-packages\nimport site\n", encoding="utf-8")
        for resource in resources:
            extract(download(resource, cache), runtime / "nltk_data" / resource["directory"])
        for directory in DIRECTORIES:
            (package / "workspace" / directory).mkdir(parents=True)
        if executable:
            shutil.copy2(executable, package / "AnnifManager.exe")
        shutil.copy2(lock, package / lock.name)
        shutil.copy2(CONFIG / "python-win64.json", package / "python-win64.json")
        shutil.copy2(CONFIG / "nltk-data.json", package / "nltk-data.json")
        shutil.copy2(CONFIG / "verify_runtime.py", package / "verify_runtime.py")
        shutil.copytree(CONFIG / "examples", package / "examples")
        for guide in ["GUIA-USO.md", "TESTE-WINDOWS-LIMPO.md"]:
            shutil.copy2(CONFIG / guide, package / guide)
        # Preserva licenças nos wheels e oferece cópias reunidas para consulta.
        licenses = package / "licenses"
        for distribution in packages.glob("*.dist-info"):
            for path in distribution.rglob("*"):
                if path.is_file() and any(word in path.name.lower() for word in ["license", "copying", "notice"]):
                    target = licenses / distribution.name / path.relative_to(distribution)
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(path, target)
        verification = verify(package, cache, args.wine)
        manifest = {
            "artifact_kind": "runtime_only" if args.runtime_only else "desktop_application",
            "python": python,
            "requirements_sha256": digest(lock),
            "nltk_resources": resources,
            "uv": subprocess.check_output([uv, "--version"], text=True).strip(),
            "dependencies": sorted(
                [{"name": item.metadata["Name"], "version": item.version}
                 for item in importlib.metadata.distributions(path=[str(packages)])],
                key=lambda item: item["name"].lower(),
            ),
            "verification": verification,
            "application_sha256": digest(package / "AnnifManager.exe") if executable else None,
        }
        (package / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        filename = "AnnifManager-runtime-win64.zip" if args.runtime_only else "AnnifManager-win64.zip"
        archive = Path(temporary) / filename
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as output:
            for path in sorted(package.rglob("*")):
                output.write(path, path.relative_to(package.parent))
        final = args.output_dir / filename
        archive.replace(final)
        final.with_suffix(".zip.sha256").write_text(f"{digest(final)}  {filename}\n", encoding="utf-8")
        print(f"Pacote criado: {final.resolve()}")
        print(f"Verificação do runtime: {verification['status']}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime-only", action="store_true", help="Gera apenas o runtime, sem aplicação desktop")
    parser.add_argument("--exe", type=Path, help="Executável Windows x64 previamente compilado em release")
    parser.add_argument("--wine", action="store_true", help="Valida o runtime Windows com Wine no Linux")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "dist")
    parser.add_argument("--cache", type=Path, default=ROOT / "target" / "packaging-cache")
    args = parser.parse_args()
    if args.runtime_only and args.exe:
        parser.error("--runtime-only e --exe são mutuamente exclusivos")
    try:
        build(args)
    except (OSError, RuntimeError, subprocess.SubprocessError, zipfile.BadZipFile) as error:
        print(f"Erro no empacotamento: {error}", file=sys.stderr)
        sys.exit(1)
