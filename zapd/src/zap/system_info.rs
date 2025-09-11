use serde::Deserialize;
/// System Info
/// 
/// 
use sysinfo::{
    Disks, Networks, System,
};

#[derive(Debug,Deserialize,Default)]
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
    
}


#[derive(Deserialize,Debug,Default)]
pub struct DiskInfo {
    pub name : String,
    pub file_system : String,
    pub available_space : u64,
}


#[derive(Deserialize,Debug,Default)]
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

    sinfo

}