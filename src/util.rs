use std::path::PathBuf;

pub fn get_current_exe_path() -> PathBuf {
    std::env::args().next().map(PathBuf::from)
        .unwrap_or_else(|| {
            debug_log!("Couldn't get current exe path from args, trying to get it from env");
            std::env::current_exe().expect("Should be able to get current exe path")
        })
}

pub fn load_env_file() {
    #[cfg(debug_assertions)] {
        dotenvy::from_path_override(".env").ok();
    }
}