// What a dependent's build script sees of nsh-core's `inspect`.
fn main() {
    let seen = std::env::var("DEP_NSH_CORE_INSPECT").unwrap_or_default();
    println!("cargo::warning=DEP_NSH_CORE_INSPECT=[{seen}]");
}
