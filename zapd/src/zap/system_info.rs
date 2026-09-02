use crate::{db, zap::ZapJsonResult};
use axum::Json;
use chrono::Duration;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration as StdDuration, Instant};

use human_bytes::human_bytes;
/// System Info
///
///
use sysinfo::{Disks, Networks, Product, System};
use systemstat::Platform;

#[derive(Debug, Deserialize, Default, Serialize)]
pub struct SystemInfo {
    pub total_memory: u64,
    pub used_memory: u64,
    pub total_swap: u64,
    pub used_swap: u64,

    pub name: String,
    pub kernel_version: String,
    pub os_version: String,
    pub host_name: String,

    pub cpu_num: usize,

    pub disks: Vec<DiskInfo>,

    pub networks: Vec<NetWorkInfo>,

    pub loadavg: (f32, f32, f32),

    pub uptime: String,

    pub boot_time: String,
}

#[derive(Deserialize, Debug, Default, Serialize)]
pub struct OsInfo {
    pub name: String,
    pub kernel_version: String,
    pub os_version: String,
    pub host_name: String,
    pub uptime: String,
    pub boot_time: String,
}

#[derive(Deserialize, Debug, Default, Serialize)]
pub struct MemoryInfo {
    pub total_memory: u64,
    pub used_memory: u64,
    pub total_swap: u64,
    pub used_swap: u64,
}

#[derive(Deserialize, Debug, Default, Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub file_system: String,
    pub available_space: u64,
    pub total_space: u64,
    pub mount_point: String,
}

#[derive(Deserialize, Debug, Default, Serialize)]
pub struct NetWorkInfo {
    pub interface_name: String,
    pub mtu: u64,
    // B bit
    pub up: u64,
    // B bit
    pub down: u64,

    pub ipaddrs: Vec<String>,
}

pub async fn get_system_info() -> ZapJsonResult {
    let mut sys = System::new_all();
    let load_avg = System::load_average();
    let pub_ip_address = match public_ip_address::perform_lookup(None).await {
        Ok(ip) => ip.ip.to_string(),
        Err(_) => match local_ip_address::local_ip() {
            Ok(ip) => ip.to_string(),
            Err(_) => "127.0.0.1".to_string(),
        },
    };

    let mut disk_info: Vec<DiskInfo> = Vec::new();
    let disks = Disks::new_with_refreshed_list();
    sys.refresh_cpu_usage();
    for disk in &disks {
        // disk.name();
        disk_info.push(DiskInfo {
            name: disk.name().to_string_lossy().to_string(),
            file_system: disk.file_system().to_string_lossy().to_string(),
            available_space: disk.available_space(),
            total_space: disk.total_space(),
            mount_point: disk.mount_point().to_string_lossy().to_string(),
        });
    }

    let systat = systemstat::System::new();
    let uptime = match systat.uptime() {
        Ok(uptime) => humantime::format_duration(uptime).to_string(),
        Err(_) => "0 s".to_string(),
    };

    let boot_time = systat.boot_time().unwrap();

    let boot_time = format!("{} {}", boot_time.date(), boot_time.time());

    return Ok(Json(json!({
        "code":0,
        "message":"OK",
        "data": {
            "os_name": System::name().unwrap_or("".to_string()),
            "os_name_version": System::long_os_version().unwrap_or("".to_string()),
            "os_id":System::distribution_id(),
            "host_name":System::host_name().unwrap_or("".to_string()),
            "arch":System::cpu_arch(),
            "physical_core_count":System::physical_core_count(),
            "cpu_num":sys.cpus().len(),
            "product_name":Product::name().unwrap_or("".to_string()),
            "product_vender_name":Product::vendor_name().unwrap_or("".to_string()),
            "uptime":uptime,
            "boot_time":boot_time,

            // 初始化 cpu / memory / disk / loadavg
            "memory_total": human_bytes(sys.total_memory() as f64),
            "memory_total_b":sys.total_memory(),
            "memory_free_b":sys.free_memory(),
            "available_memory_b":sys.available_memory(),
            "swap_total":sys.total_swap(),
            "swap_used":sys.used_swap(),
            "swap_free":sys.free_swap(),

            "cpu_usage":sys.global_cpu_usage(),

            "loadavg_one": load_avg.one,
            "loadavg_five": load_avg.five,
            "loadavg_fifteen": load_avg.fifteen,
            "public_ip":pub_ip_address,

            "disk_info": disk_info,
        }
    })));
}

pub async fn get_system_status() -> ZapJsonResult {
    let mut sys = System::new_all();
    let load_avg = System::load_average();

    let systat = systemstat::System::new();
    let uptime = match systat.uptime() {
        Ok(uptime) => humantime::format_duration(uptime).to_string(),
        Err(_) => "0 s".to_string(),
    };
    let current_time = chrono::Local::now();
    let five_algo = current_time - Duration::minutes(5);

    let pool = db::get_db_pool().await;
    let system_stats: Vec<db::models::SystemStatsModel> =
        sqlx::query_as("select * from system_stats where created_at >= $1")
            .bind(five_algo.timestamp())
            .fetch_all(pool)
            .await?;
    let network_stats: Vec<db::models::NetworksStatsForDashboard> = sqlx::query_as(
        "select name, received,transmitted,packets_received,
    packets_transmitted,total_received,total_transmitted,
    ipaddrs,created_at from networks_stats where created_at >= $1",
    )
    .bind(five_algo.timestamp())
    .fetch_all(pool)
    .await?;
    sys.refresh_cpu_usage();
    return Ok(Json(json!({
        "code":0,
        "message":"OK",
        "data": {
            "uptime":uptime,

            // 初始化 cpu / memory / disk / loadavg
            "memory_total": human_bytes(sys.total_memory() as f64),
            "memory_total_b":sys.total_memory(),
            "memory_free_b":sys.free_memory(),
            "available_memory_b":sys.available_memory(),
            "swap_total":sys.total_swap(),
            "swap_used":sys.used_swap(),
            "swap_free":sys.free_swap(),

            "cpu_usage":sys.global_cpu_usage(),

            "loadavg_one": load_avg.one,
            "loadavg_five": load_avg.five,
            "loadavg_fifteen": load_avg.fifteen,
            "system_stats" : system_stats,
            "network_stats" : network_stats,
        }
    })));
}

pub async fn get_os_info() -> SystemInfo {
    let mut sinfo = SystemInfo::default();

    let mut sys = System::new_all();

    // First we update all information of our `System` struct.
    sys.refresh_all();

    sinfo.total_memory = sys.total_memory();
    sinfo.used_memory = sys.used_memory();
    sinfo.total_swap = sys.total_swap();
    sinfo.used_swap = sys.used_swap();

    sinfo.name = System::name().unwrap_or("".to_string());
    sinfo.kernel_version = System::kernel_version().unwrap_or("".to_string());
    sinfo.os_version = System::os_version().unwrap_or("".to_string());
    sinfo.host_name = System::host_name().unwrap_or("".to_string());

    sinfo.cpu_num = sys.cpus().len();

    // Display processes ID, name and disk usage:
    // for (pid, process) in sys.processes() {
    // println!("[{pid}] {:?} {:?}", process.name(), process.disk_usage());
    // }

    // We display all disks' information:
    sinfo.disks = Vec::new();
    let disks = Disks::new_with_refreshed_list();
    for disk in &disks {
        // disk.name();
        sinfo.disks.push(DiskInfo {
            name: disk.name().to_string_lossy().to_string(),
            file_system: disk.file_system().to_string_lossy().to_string(),
            available_space: disk.available_space(),
            total_space: disk.total_space(),
            mount_point: disk.mount_point().to_string_lossy().to_string(),
        });
    }

    // Network interfaces name, total data received and total data transmitted:
    let networks = Networks::new_with_refreshed_list();

    for (interface_name, data) in &networks {
        let ip = data.ip_networks().iter().map(|v| v.to_string()).collect();

        sinfo.networks.push(NetWorkInfo {
            interface_name: interface_name.to_string(),
            mtu: data.mtu(),
            up: data.total_transmitted(),
            down: data.total_received(),
            ipaddrs: ip,
        });
    }
    let systat = systemstat::System::new();
    sinfo.uptime = match systat.uptime() {
        Ok(uptime) => humantime::format_duration(uptime).to_string(),
        Err(_) => "0 s".to_string(),
    };

    let boot_time = systat.boot_time().unwrap();

    sinfo.boot_time = format!("{} {}", boot_time.date(), boot_time.time());

    sinfo.loadavg = match systat.load_average() {
        Ok(loadavg) => (loadavg.one, loadavg.five, loadavg.fifteen),
        Err(_) => (0.0, 0.0, 0.0),
    };
    sinfo
}

/// 服务器信息概览 —— 内存条（dmidecode 尽力而为）
#[derive(Clone, Deserialize, Debug, Default, Serialize)]
pub struct MemModule {
    pub size: String,
    pub locator: String,
    pub bank_locator: String,
    pub memory_type: String,
    pub speed: String,
    pub manufacturer: String,
    pub part_number: String,
}

/// 服务器信息概览 —— 物理磁盘
#[derive(Deserialize, Debug, Default, Serialize)]
pub struct PhysicalDisk {
    pub device: String,
    pub model: String,
    pub size: u64,
    pub rotational: bool,
    pub interface: String,
}

/// 从 /proc/cpuinfo 读取 CPU 型号
fn cpu_model_name() -> String {
    if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("model name") {
                if let Some(name) = rest.split(':').nth(1) {
                    let name = name.trim();
                    if !name.is_empty() {
                        return name.to_string();
                    }
                }
            }
        }
    }
    String::new()
}

/// 从 /proc/cpuinfo 读取第一个 CPU 核当前频率（MHz）
fn cpu_frequency_mhz() -> u64 {
    if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("cpu MHz") {
                if let Some(mhz) = rest
                    .split(':')
                    .nth(1)
                    .and_then(|s| s.trim().parse::<f64>().ok())
                {
                    return mhz.round() as u64;
                }
            }
        }
    }
    0
}

/// 枚举物理磁盘（/sys/block，跳过 loop/ram 等虚拟设备）
fn list_physical_disks() -> Vec<PhysicalDisk> {
    let mut disks: Vec<PhysicalDisk> = Vec::new();
    let Ok(entries) = std::fs::read_dir("/sys/block") else {
        return disks;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("loop")
            || name.starts_with("ram")
            || name.starts_with("fd")
            || name.starts_with("zram")
            || name.starts_with("dm-")
            || name.starts_with("sr")
        {
            continue;
        }
        // 可移动设备（软驱/U 盘等）跳过
        if let Ok(removable) = std::fs::read_to_string(format!("/sys/block/{name}/removable")) {
            if removable.trim() == "1" {
                continue;
            }
        }
        let model = std::fs::read_to_string(format!("/sys/block/{name}/device/model"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_default();
        let size = std::fs::read_to_string(format!("/sys/block/{name}/size"))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|sectors| sectors * 512)
            .unwrap_or(0);
        let rotational = std::fs::read_to_string(format!("/sys/block/{name}/queue/rotational"))
            .ok()
            .map(|s| s.trim() == "1")
            .unwrap_or(false);
        let interface = if name.starts_with("nvme") {
            "NVMe"
        } else if name.starts_with("sd") {
            "SATA/SCSI"
        } else if name.starts_with("vd") {
            "VirtIO"
        } else if name.starts_with("mmc") {
            "eMMC/SD"
        } else {
            "Other"
        };
        disks.push(PhysicalDisk {
            device: name,
            model,
            size,
            rotational,
            interface: interface.to_string(),
        });
    }
    disks
}

/// 解析 dmidecode -t 17 输出，提取内存条信息
fn parse_dmidecode_mem(text: &str) -> Vec<MemModule> {
    let mut modules: Vec<MemModule> = Vec::new();
    let mut cur: Option<MemModule> = None;
    for line in text.lines() {
        if line.trim_start().starts_with("Memory Device") {
            if let Some(m) = cur.take() {
                modules.push(m);
            }
            cur = Some(MemModule::default());
            continue;
        }
        if let Some(m) = cur.as_mut() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();
                match key {
                    "Size" => m.size = value.to_string(),
                    "Locator" => m.locator = value.to_string(),
                    "Bank Locator" => m.bank_locator = value.to_string(),
                    "Type" => m.memory_type = value.to_string(),
                    "Speed" => m.speed = value.to_string(),
                    "Manufacturer" => m.manufacturer = value.to_string(),
                    "Part Number" => m.part_number = value.to_string(),
                    _ => {}
                }
            }
        }
    }
    if let Some(m) = cur {
        modules.push(m);
    }
    modules
}

const MEM_CACHE_TTL: StdDuration = StdDuration::from_secs(60);
static MEM_MODULES_CACHE: OnceLock<Mutex<(Instant, Vec<MemModule>)>> = OnceLock::new();

/// 读取内存条信息（dmidecode 存在且执行成功才有数据，60s 缓存）
async fn mem_modules_cached() -> Vec<MemModule> {
    let cell = MEM_MODULES_CACHE.get_or_init(|| {
        Mutex::new((
            Instant::now() - StdDuration::from_secs(MEM_CACHE_TTL.as_secs() * 2),
            Vec::new(),
        ))
    });
    {
        let guard = cell.lock().unwrap();
        if guard.0.elapsed() < MEM_CACHE_TTL {
            return guard.1.clone();
        }
    }
    let modules = match tokio::process::Command::new("dmidecode")
        .args(["-t", "17"])
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            parse_dmidecode_mem(&String::from_utf8_lossy(&output.stdout))
        }
        _ => Vec::new(),
    };
    let mut guard = cell.lock().unwrap();
    *guard = (Instant::now(), modules.clone());
    modules
}

/// 服务器信息概览：系统 / 处理器 / 内存 / 物理磁盘 / 使用率
pub async fn get_system_overview() -> ZapJsonResult {
    let mut sys = System::new_all();
    let load_avg = System::load_average();
    sys.refresh_cpu_usage();

    let systat = systemstat::System::new();
    let uptime = match systat.uptime() {
        Ok(uptime) => humantime::format_duration(uptime).to_string(),
        Err(_) => "0 s".to_string(),
    };
    let boot_time = match systat.boot_time() {
        Ok(bt) => format!("{} {}", bt.date(), bt.time()),
        Err(_) => String::new(),
    };
    let current_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // 分区使用情况
    let mut disk_usage: Vec<serde_json::Value> = Vec::new();
    let disks = Disks::new_with_refreshed_list();
    for disk in &disks {
        let total = disk.total_space();
        let available = disk.available_space();
        let used = total.saturating_sub(available);
        let usage_pct = if total > 0 {
            (used as f64 / total as f64 * 1000.0).round() / 10.0
        } else {
            0.0
        };
        disk_usage.push(json!({
            "name": disk.name().to_string_lossy().to_string(),
            "file_system": disk.file_system().to_string_lossy().to_string(),
            "mount_point": disk.mount_point().to_string_lossy().to_string(),
            "total": total,
            "used": used,
            "available": available,
            "usage_pct": usage_pct,
        }));
    }

    let total_memory = sys.total_memory();
    let available_memory = sys.available_memory();
    let used_memory = total_memory.saturating_sub(available_memory);
    let mem_usage_pct = if total_memory > 0 {
        (used_memory as f64 / total_memory as f64 * 1000.0).round() / 10.0
    } else {
        0.0
    };

    Ok(Json(json!({
        "code": 0,
        "message": "OK",
        "data": {
            "host_name": System::host_name().unwrap_or_default(),
            "os_name": System::name().unwrap_or_default(),
            "os_version": System::long_os_version().unwrap_or_default(),
            "kernel_version": System::kernel_version().unwrap_or_default(),
            "arch": System::cpu_arch(),
            "vendor": Product::vendor_name().unwrap_or_default(),
            "product": Product::name().unwrap_or_default(),
            "uptime": uptime,
            "boot_time": boot_time,
            "current_time": current_time,
            "cpu": {
                "model": cpu_model_name(),
                "physical_cores": System::physical_core_count().unwrap_or(0),
                "logical_cores": sys.cpus().len(),
                "frequency_mhz": cpu_frequency_mhz(),
                "usage": sys.global_cpu_usage(),
                "loadavg_one": load_avg.one,
                "loadavg_five": load_avg.five,
                "loadavg_fifteen": load_avg.fifteen,
            },
            "memory": {
                "total": total_memory,
                "modules": mem_modules_cached().await,
            },
            "memory_usage": {
                "total": total_memory,
                "used": used_memory,
                "free": sys.free_memory(),
                "available": available_memory,
                "usage_pct": mem_usage_pct,
                "swap_total": sys.total_swap(),
                "swap_used": sys.used_swap(),
                "swap_free": sys.free_swap(),
            },
            "physical_disks": list_physical_disks(),
            "disk_usage": disk_usage,
        }
    })))
}
