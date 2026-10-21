//! What the title screen needs to know about this computer: OS, home folder, disks.

use serde::Serialize;
use sysinfo::Disks;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub name: String,
    pub mount: String,
    pub total: u64,
    pub available: u64,
    pub removable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub platform: &'static str,
    pub home: Option<String>,
    pub volumes: Vec<Volume>,
    /// What "scan the whole computer" means here: `/` on macOS, every fixed drive on Windows.
    pub whole_disk_roots: Vec<String>,
}

pub fn system_info() -> SystemInfo {
    let volumes = volumes();
    let whole_disk_roots = if cfg!(windows) {
        volumes.iter().filter(|v| !v.removable).map(|v| v.mount.clone()).collect()
    } else {
        vec!["/".to_string()]
    };
    SystemInfo {
        platform: if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(windows) {
            "windows"
        } else {
            "linux"
        },
        home: dirs::home_dir().map(|h| h.to_string_lossy().into_owned()),
        volumes,
        whole_disk_roots,
    }
}

fn volumes() -> Vec<Volume> {
    let disks = Disks::new_with_refreshed_list();
    let mut out: Vec<Volume> = Vec::new();
    for disk in disks.list() {
        let mount = disk.mount_point().to_string_lossy().into_owned();
        if disk.total_space() == 0 || !is_user_facing(&mount) || out.iter().any(|v| v.mount == mount) {
            continue;
        }
        let name = disk.name().to_string_lossy().into_owned();
        out.push(Volume {
            name: if name.is_empty() { mount.clone() } else { name },
            mount,
            total: disk.total_space(),
            available: disk.available_space(),
            removable: disk.is_removable(),
        });
    }
    out
}

/// Hides the many internal mounts (APFS system volumes, snap loops...) people don't think of as disks.
fn is_user_facing(mount: &str) -> bool {
    if cfg!(target_os = "macos") {
        mount == "/" || mount.starts_with("/Volumes/")
    } else if cfg!(windows) {
        true
    } else {
        mount == "/" || ["/home", "/mnt/", "/media/"].iter().any(|p| mount.starts_with(p))
    }
}
