"""Click CLI entry point."""

from __future__ import annotations

import click

from slurm_docker import __version__


@click.group()
@click.version_option(version=__version__, prog_name="docker")
def cli() -> None:
    """Slurm Docker management utilities."""


@cli.command()
def hello() -> None:
    """Print a greeting."""
    click.echo("Hello from slurm-docker!")


def main() -> None:
    """Console script entry point."""
    cli()
