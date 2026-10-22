"""CLI entry point for the pinned Annif 1.4.2 portable runtime."""
import os
import types

if os.name == "nt":
    import annif.util

    # atomic_save expects POSIX rename semantics (replace an existing target).
    # Windows os.rename raises WinError 183 on reload/retraining. Scope the
    # adaptation to Annif's utility module; do not mutate the global os module.
    annif.util.os = types.SimpleNamespace(**vars(os))
    annif.util.os.rename = os.replace

from annif.cli import cli

cli()
