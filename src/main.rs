fn main() {
    if let Err(err) = slurm_docker::cli::main() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
