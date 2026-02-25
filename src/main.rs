use std::env;
use std::process::{Command, exit};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Cannot open links if no arguments were provided");
        exit(1);
    }

    let script_path = r#"D:\Geral\Scripts\firefox_router.ps1"#;

    let mut command = Command::new("powershell.exe");

    command.arg("-ExecutionPolicy").arg("Bypass").arg("-File").arg(script_path);

    for arg in &args {
        command.arg(arg);
    }

    let status = match command.status() {
        Ok(status) => status,
        Err(_) => {
            eprintln!("An error occurred");
            exit(1);
        }
    };

    if !status.success() {
        exit(status.code().unwrap_or(1));
    }
}