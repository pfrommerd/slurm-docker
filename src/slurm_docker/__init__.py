"""Slurm Docker management utilities."""

from importlib.metadata import PackageNotFoundError, version

__all__ = ["__version__"]


def _package_version() -> str:
    try:
        return version("slurm-docker")
    except PackageNotFoundError:
        return "0.0.0+unknown"


__version__ = _package_version()
