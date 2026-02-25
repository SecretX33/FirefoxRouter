#![windows_subsystem = "windows"]

use regex::Regex;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::{Child, Command};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
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

    match args.first().map(|s| s.as_str()) {
        Some("--register") => register(),
        Some("--unregister") => unregister(),
        _ => handle_link(args)
    }
}

fn handle_link(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
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

fn register() -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = std::env::current_exe()?.to_string_lossy().into_owned();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    log!("Current exe path: {exe_path}");

    // ProgID for URL handling
    let (url_class, _) = hkcu.create_subkey(r"SOFTWARE\Classes\FirefoxRouterURL")?;
    url_class.set_value("", &"FirefoxRouter URL")?;
    url_class.set_value("URL Protocol", &"")?;
    let (url_icon, _) = hkcu.create_subkey(r"SOFTWARE\Classes\FirefoxRouterURL\DefaultIcon")?;
    url_icon.set_value("", &format!("{exe_path},0"))?;
    let (url_cmd, _) = hkcu.create_subkey(r"SOFTWARE\Classes\FirefoxRouterURL\shell\open\command")?;
    url_cmd.set_value("", &format!("\"{exe_path}\" \"%1\""))?;

    // ProgID for HTML file handling
    let (html_class, _) = hkcu.create_subkey(r"SOFTWARE\Classes\FirefoxRouterHTML")?;
    html_class.set_value("", &"FirefoxRouter HTML Document")?;
    let (html_icon, _) = hkcu.create_subkey(r"SOFTWARE\Classes\FirefoxRouterHTML\DefaultIcon")?;
    html_icon.set_value("", &format!("{exe_path},0"))?;
    let (html_cmd, _) = hkcu.create_subkey(r"SOFTWARE\Classes\FirefoxRouterHTML\shell\open\command")?;
    html_cmd.set_value("", &format!("\"{exe_path}\" \"%1\""))?;

    // StartMenuInternet client
    let (client, _) = hkcu.create_subkey(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter")?;
    client.set_value("", &"FirefoxRouter")?;
    let (caps, _) = hkcu.create_subkey(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter\Capabilities")?;
    caps.set_value("ApplicationName", &"FirefoxRouter")?;
    caps.set_value("ApplicationDescription", &"Routes URLs to Firefox using the active profile")?;
    let (url_assoc, _) = hkcu.create_subkey(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter\Capabilities\URLAssociations")?;
    url_assoc.set_value("http", &"FirefoxRouterURL")?;
    url_assoc.set_value("https", &"FirefoxRouterURL")?;
    let (file_assoc, _) = hkcu.create_subkey(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter\Capabilities\FileAssociations")?;
    file_assoc.set_value(".htm", &"FirefoxRouterHTML")?;
    file_assoc.set_value(".html", &"FirefoxRouterHTML")?;
    let (start_menu, _) = hkcu.create_subkey(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter\Capabilities\StartMenu")?;
    start_menu.set_value("StartMenuInternet", &"FirefoxRouter")?;
    let (client_icon, _) = hkcu.create_subkey(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter\DefaultIcon")?;
    client_icon.set_value("", &format!("{exe_path},0"))?;
    let (client_cmd, _) = hkcu.create_subkey(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter\shell\open\command")?;
    client_cmd.set_value("", &format!("\"{exe_path}\""))?;

    // RegisteredApplications entry
    let (reg_apps, _) = hkcu.create_subkey(r"SOFTWARE\RegisteredApplications")?;
    reg_apps.set_value("FirefoxRouter", &r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter\Capabilities")?;

    log!("FirefoxRouter registered as a browser. Open Settings > Default Apps to set it as default.");
    Ok(())
}

fn unregister() -> Result<(), Box<dyn std::error::Error>> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    // Remove ProgIDs
    let _ = hkcu.delete_subkey_all(r"SOFTWARE\Classes\FirefoxRouterURL");
    let _ = hkcu.delete_subkey_all(r"SOFTWARE\Classes\FirefoxRouterHTML");

    // Remove StartMenuInternet client
    let _ = hkcu.delete_subkey_all(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter");

    // Remove RegisteredApplications entry
    if let Ok(reg_apps) = hkcu.open_subkey(r"SOFTWARE\RegisteredApplications") {
        let _ = reg_apps.delete_value("FirefoxRouter");
    }

    log!("FirefoxRouter unregistered.");
    Ok(())
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
