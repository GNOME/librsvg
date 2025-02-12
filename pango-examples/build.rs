fn probe_system_deps() {
    if let Err(e) = system_deps::Config::new().probe() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn main() {
    probe_system_deps();
}
