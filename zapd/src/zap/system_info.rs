use serde::{Deserialize, Serialize};
/// System Info
/// 
/// 
use sysinfo::{
    Disks, Networks, System,
};
use systemstat::Platform;

#[derive(Debug,Deserialize,Default,Serialize)]
pub struct SystemInfo {
    pub total_memory : u64,
    pub used_memory : u64,
    pub total_swap : u64,
    pub used_swap : u64,

    pub name : String,
    pub kernel_version : String,
    pub os_version : String,
    pub host_name : String,

    pub cpu_num : usize,

    pub disks: Vec<DiskInfo>,

    pub networks: Vec<NetWorkInfo>,

    pub loadavg : (f32,f32,f32),

    pub uptime : String,

    pub boot_time : String,
    
}


#[derive(Deserialize,Debug,Default,Serialize)]
pub struct DiskInfo {
    pub name : String,
    pub file_system : String,
    pub available_space : u64,
    pub total_space : u64,
    pub mount_point : String,
}


#[derive(Deserialize,Debug,Default,Serialize)]
pub struct NetWorkInfo {
    pub interface_name : String,
    pub mtu : u64,
    // B bit
    pub up : u64,
    // B bit
    pub down : u64,

    pub ipaddrs : Vec<String>,
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
        sinfo.disks.push(DiskInfo{
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
        let ip =  data.ip_networks().iter().map(|v| v.to_string()).collect();
        
        sinfo.networks.push(NetWorkInfo{
            interface_name: interface_name.to_string(),
            mtu : data.mtu(),
            up : data.total_transmitted(),
            down : data.total_received(),
            ipaddrs : ip,
        });
    }
    let systat = systemstat::System::new();
    sinfo.uptime = match systat.uptime() {
        Ok(uptime)=> humantime::format_duration(uptime).to_string(),
        Err(_) => "0 s".to_string(),
    };

    let boot_time = systat.boot_time().unwrap();
    
    
    sinfo.boot_time = format!("{} {}",boot_time.date(),boot_time.time());
    
    sinfo.loadavg = match systat.load_average() {
        Ok(loadavg) => (loadavg.one,loadavg.five,loadavg.fifteen),
        Err(_) => (0.0,0.0,0.0)
    };
    sinfo

}