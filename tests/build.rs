use pkg_config;

fn main() {
    check_for_pangoft2();
}

fn check_for_pangoft2() {
    if pkg_config::Config::new().atleast_version("1.38").probe("pangoft2").is_ok() {
        println!("cargo:rustc-cfg=have_pangoft2");
    }
}
