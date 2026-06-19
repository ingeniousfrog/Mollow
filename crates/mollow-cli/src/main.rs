mod app;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    match app::execute(&app::Cli::parse()) {
        Ok(output) => {
            if let Some(path) = output.path {
                if let Err(error) = std::fs::write(&path, output.content) {
                    eprintln!("mollow: failed to write {}: {error}", path.display());
                    return ExitCode::FAILURE;
                }
            } else {
                print!("{}", output.content);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("mollow: {error}");
            ExitCode::FAILURE
        }
    }
}
