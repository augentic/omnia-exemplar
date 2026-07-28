//! Stand-alone entry point for local iteration; CI runs the same gate
//! through `tests/gate.rs`.

use std::process::ExitCode;

fn main() -> ExitCode {
    let root = template_check::repo_root();
    match template_check::run(&root) {
        Ok(failures) if failures.is_empty() => {
            println!("template-check: ok");
            ExitCode::SUCCESS
        }
        Ok(failures) => {
            for failure in &failures {
                eprintln!("template-check: {failure}");
            }
            eprintln!("template-check: {} failure(s)", failures.len());
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("template-check: {err}");
            ExitCode::FAILURE
        }
    }
}
