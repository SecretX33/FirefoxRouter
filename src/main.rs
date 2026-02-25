use regex::Regex;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::{Child, Command};
use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;
use wmi::WMIConnection;

#[macro_use]
mod log_macro;

#[derive(Debug, Deserialize)]
#[serde(rename = "Win32_Process")]
#[serde(rename_all = "PascalCase")]
struct Win32Process {
    process_id: u32,
    command_line: Option<String>,
}

#[cfg(windows)]
fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    debug_log!("URL: {:?}", args);

    let wmi = WMIConnection::new()?;

    let processes: Vec<Win32Process> = wmi.raw_query(
        "SELECT ProcessId, CommandLine FROM Win32_Process WHERE Name = 'firefox.exe'",
    )?;

    // Inspect each Firefox process for a profile flag
    let re_profile = Regex::new(r#"-profile\s+"?([^"]+)"?"#)?;
    let re_dash_p = Regex::new(r#"-P\s+"?([^"]+)"?"#)?;

    for proc in &processes {
        let cmd = match &proc.command_line {
            Some(c) if !c.trim().is_empty() => c,
            _ => continue,
        };

        let matcher = re_profile.captures(cmd)
            .or_else(|| re_dash_p.captures(cmd));

        // Check for `-P <profileName>` and `-profile <profilePath>`
        if let Some(caps) = matcher {
            let process_id = proc.process_id;
            let profile_name = caps.get(1).unwrap().as_str();
            debug_log!("Found Firefox with profile currently in use by {process_id}: {profile_name}");
            open_with_firefox(args, Some(profile_name))?;
            return Ok(());
        }
    }

    debug_log!("Didn't spot any Firefox with profile currently in use, opening link in the default profile");
    open_with_firefox(args, None)?;

    Ok(())
}

fn open_with_firefox(
    args: Vec<String>,
    profile_name: Option<&str>,
) -> std::io::Result<Child> {
    let firefox_path = find_firefox();
    debug_log!("Using Firefox at: {}", firefox_path.display());

    let mut command = Command::new(&firefox_path);
    if let Some(profile_name) = profile_name {
        command.arg("-P").arg(profile_name);
    }
    for arg in &args {
        command.arg(arg);
    }
    command.spawn()
}

fn find_firefox() -> PathBuf {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let result = hklm.open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\firefox.exe")
        .and_then(|it| it.get_value::<String, _>(""));
    if let Ok(path) = result {
        return PathBuf::from(path);
    }
    // Last resort: hope it's on PATH
    PathBuf::from("firefox.exe")
}
