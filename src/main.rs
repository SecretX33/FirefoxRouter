#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::config::{load_env_file, read_app_config};
use color_eyre::Result;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use sysinfo::{Process, System};
use winreg::enums::KEY_ALL_ACCESS;

#[macro_use]
mod log_macro;
mod config;
mod glob;

#[derive(Debug, PartialEq, Eq)]
struct FirefoxInfo {
    path: String,
    profile_name: Option<String>,
}

impl PartialOrd for FirefoxInfo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FirefoxInfo {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let profile_cmp = match (&self.profile_name, &other.profile_name) {
            (Some(self_profile), Some(other_profile)) => self_profile.cmp(other_profile),
            (a, b) => b.cmp(a),
        };
        profile_cmp.then_with(|| self.path.cmp(&other.path))
    }
}

fn main() -> Result<()> {
    load_env_file();
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(|s| s.as_str()) {
        Some("--register") => register(),
        Some("--unregister") => unregister(),
        _ => handle_links(args)
    }
}

fn handle_links(args: Vec<String>) -> Result<()> {
    debug_log!("Args: {:?}", args);

    let args = filter_args(&args)?;
    if args.len() == 0 {
        debug_log!("All URLs got filtered out, nothing to do");
        return Ok(());
    }

    let sys = System::new_all();
    let processes = sys.processes().values();

    let mut firefox_processes = processes
        .filter(|it| is_firefox_process(it))
        .filter_map(|it| get_firefox_info(it))
        .collect::<Vec<_>>();

    firefox_processes.sort();

    if firefox_processes.len() == 0 {
        debug_log!("No Firefox processes found, opening link in the default profile");
        open_with_firefox(args, None)?;
        return Ok(());
    }

    let first_info = firefox_processes.first().unwrap();
    if first_info.profile_name.is_some() {
        debug_log!("Found existing Firefox process with an active profile");
    } else {
        debug_log!("Didn't spot any Firefox with profile currently in use, opening link in the default profile");
    }

    open_with_firefox(args, Some(first_info))?;
    Ok(())
}

fn filter_args(args: impl IntoIterator<Item = impl AsRef<str>>) -> Result<Vec<String>> {
    let Some(config) = read_app_config()? else {
        debug_log!("No config file found, not filtering URLs");
        return Ok(args.into_iter().map(|s| s.as_ref().to_owned()).collect());
    };

    let args: Vec<String> = args.into_iter().map(|s| s.as_ref().to_owned()).collect();
    let filtered_args: Vec<_> = args.iter().filter(|&url| {
        config.ignored_urls.iter().all(|it| !it.is_match(url))
            && config.ignored_urls_regex.iter().all(|it| !it.as_ref().is_match(url))
    }).cloned().collect();

    if filtered_args.len() != args.len() {
        debug_log!(
            "Removed {} URLs from the list due to configured URL filtering rules ({} -> {})",
            args.len() - filtered_args.len(),
            args.len(),
            filtered_args.len()
        );
    }
    Ok(filtered_args)
}

fn is_firefox_process(it: &Process) -> bool {
    it.cmd().first()
        .and_then(|s| {
            Path::new(s)
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.eq_ignore_ascii_case("firefox.exe"))
        })
        .unwrap_or(false)
}

fn get_firefox_info(it: &Process) -> Option<FirefoxInfo> {
    let cmd = it.cmd();
    if cmd.len() == 0 {
        debug_log!("Attempted to get Firefox info for a process with no command line arguments");
        return None;
    }

    let path = cmd.first().map(|s| s.to_string_lossy()).unwrap().into_owned();
    let profile_name = cmd.into_iter()
        .skip_while(|&s| s != "-P" && s != "-profile")
        .skip(1).next()
        .map(|s| s.to_string_lossy().into_owned());

    Some(FirefoxInfo {
        path,
        profile_name,
    })
}

fn open_with_firefox(
    args: Vec<String>,
    firefox_info: Option<&FirefoxInfo>,
) -> std::io::Result<()> {
    let firefox_path = firefox_info.map(|it| it.path.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(find_firefox);
    debug_log!("Using Firefox at: {}, profile: {}", firefox_path.display(), firefox_info.and_then(|it| it.profile_name.as_deref()).unwrap_or("<none>"));

    let mut command = Command::new(&firefox_path);
    if let Some(profile_name) = firefox_info.and_then(|it| it.profile_name.as_deref()) {
        command.arg("-P").arg(profile_name);
    }
    for arg in &args {
        command.arg("-url").arg(arg);
    }

    #[cfg(debug_assertions)] {
        if std::env::var("DISABLE_LINK_OPENING") == Ok("true".to_owned()) {
            debug_log!("Link opening disabled, not spawning process");
            return Ok(());
        }
    }
    command.spawn().map(|_| ())
}

fn find_firefox() -> PathBuf {
    #[cfg(windows)] {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let result = hklm.open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\firefox.exe")
            .and_then(|it| it.get_value::<String, _>(""));
        if let Ok(path) = result {
            return PathBuf::from(path);
        }
    }

    // Last resort: hope it's on PATH
    PathBuf::from("firefox.exe")
}

#[cfg(windows)]
fn register() -> Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    unregister()?;

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
    html_icon.set_value("", &format!("{exe_path},1"))?;
    let (html_cmd, _) = hkcu.create_subkey(r"SOFTWARE\Classes\FirefoxRouterHTML\shell\open\command")?;
    html_cmd.set_value("", &format!("\"{exe_path}\" \"%1\""))?;

    // StartMenuInternet client
    let (client, _) = hkcu.create_subkey(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter")?;
    client.set_value("", &"Firefox Router")?;
    let (caps, _) = hkcu.create_subkey(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter\Capabilities")?;
    caps.set_value("ApplicationName", &"Firefox Router")?;
    caps.set_value("ApplicationDescription", &"Routes URLs to Firefox using the active profile")?;
    let (file_assoc, _) = hkcu.create_subkey(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter\Capabilities\FileAssociations")?;
    file_assoc.set_value(".htm", &"FirefoxRouterHTML")?;
    file_assoc.set_value(".html", &"FirefoxRouterHTML")?;
    let (start_menu, _) = hkcu.create_subkey(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter\Capabilities\StartMenu")?;
    start_menu.set_value("StartMenuInternet", &"FirefoxRouter")?;
    let (url_assoc, _) = hkcu.create_subkey(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter\Capabilities\URLAssociations")?;
    url_assoc.set_value("http", &"FirefoxRouterURL")?;
    url_assoc.set_value("https", &"FirefoxRouterURL")?;
    let (client_icon, _) = hkcu.create_subkey(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter\DefaultIcon")?;
    client_icon.set_value("", &format!("{exe_path},0"))?;
    let (client_cmd, _) = hkcu.create_subkey(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter\shell\open\command")?;
    client_cmd.set_value("", &format!("\"{exe_path}\""))?;

    // RegisteredApplications entry
    let (reg_apps, _) = hkcu.create_subkey(r"SOFTWARE\RegisteredApplications")?;
    reg_apps.set_value("FirefoxRouter", &r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter\Capabilities")?;

    log!("FirefoxRouter registered as a browser. Open Settings > Default Apps to set it as default");
    Ok(())
}

#[cfg(windows)]
fn unregister() -> Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    // Remove ProgIDs
    let _ = hkcu.delete_subkey_all(r"SOFTWARE\Classes\FirefoxRouterURL");
    let _ = hkcu.delete_subkey_all(r"SOFTWARE\Classes\FirefoxRouterHTML");

    // Remove StartMenuInternet client
    let _ = hkcu.delete_subkey_all(r"SOFTWARE\Clients\StartMenuInternet\FirefoxRouter");

    // Remove RegisteredApplications entry
    if let Ok(reg_apps) = hkcu.open_subkey_with_flags(r"SOFTWARE\RegisteredApplications", KEY_ALL_ACCESS) {
        let _ = reg_apps.delete_value("FirefoxRouter");
    }

    log!("FirefoxRouter unregistered");
    Ok(())
}
