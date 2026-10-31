mod cli;

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

fn main() -> std::process::ExitCode {
    pwe_core::i18n::bind(&pwe_core::i18n::default_directory());
    match cli::parse(std::env::args().skip(1).collect()) {
        Ok(action) => cli::run(action),
        Err(message) => {
            eprintln!("{message}");
            std::process::ExitCode::FAILURE
        }
    }
}
