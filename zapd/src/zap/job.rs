use std::collections::HashMap;

use once_cell::sync::Lazy;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, Networks, RefreshKind, System};
use tokio::sync::RwLock;
use tokio_cron_scheduler::{job::job_data::Uuid, Job, JobScheduler};
use tracing::{debug, info};

use crate::db::get_db_pool;

static GLOBAL_SCHEDULED_MAP: Lazy<RwLock<HashMap<String, JobScheduler>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));
static GLOBAL_JOB_MAP: Lazy<RwLock<HashMap<String, Uuid>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

static GLOBAL_SYSTEM_INFO: Lazy<RwLock<HashMap<String, String>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

async fn system_scheduled_task() {
    debug!("Scheduled task executed at: {:?}", chrono::Utc::now());
    let pool = get_db_pool().await;
    let per_10s = 10; // seconds
    // load avg
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );
    let load_avg = sysinfo::System::load_average();
    sys.refresh_cpu_usage();
    let cpu_usage = sys.global_cpu_usage();
    let memory_usage = (sys.total_memory() - sys.available_memory()) as f32
        / sys.total_memory() as f32
        * 100.0;
    let swap_usage = if sys.total_swap() == 0 {
        0.00_f32
    } else {
        (sys.total_swap() - sys.free_swap()) as f32 / sys.total_swap() as f32 * 100.0
    };

    // network traffic
    let mut networks = Networks::new_with_refreshed_list();
    networks.refresh(true);
    for (interface_name, data) in networks.iter() {
        let last_received = GLOBAL_SYSTEM_INFO
            .read()
            .await
            .get(&format!("net_{}_total_received", interface_name))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let last_transmitted = GLOBAL_SYSTEM_INFO
            .read()
            .await
            .get(&format!("net_{}_total_transmitted", interface_name))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let last_packets_received = GLOBAL_SYSTEM_INFO
            .read()
            .await
            .get(&format!("net_{}_packets_received", interface_name))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let last_packets_transmitted = GLOBAL_SYSTEM_INFO
            .read()
            .await
            .get(&format!("net_{}_packets_transmitted", interface_name))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);

        let received = if data.total_received() > last_received {
            (data.total_received() - last_received) / per_10s
        } else {
            0
        };
        let transmitted = if data.total_transmitted() > last_transmitted {
            (data.total_transmitted() - last_transmitted) / per_10s
        } else {
            0
        };
        let packets_received = if data.packets_received() > last_packets_received {
            (data.packets_received() - last_packets_received) / per_10s
        } else {
            0
        };
        let packets_transmitted =
            if data.packets_transmitted() > last_packets_transmitted {
                (data.packets_transmitted() - last_packets_transmitted) / per_10s
            } else {
                0
            };
        let ip: Vec<String> = data.ip_networks().iter().map(|v| v.to_string()).collect();

        let _ = sqlx::query(
            "insert into networks_stats (name,
        received,transmitted,
        errors_on_received,errors_on_transmitted,
        packets_received,packets_transmitted,
        total_received,total_transmitted,
        total_packets_received,total_packets_transmitted,
        total_errors_on_received,total_errors_on_transmitted,
        ipaddrs,created_at) 
        values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
        )
        .bind(interface_name)
        .bind(received as i64)
        .bind(transmitted as i64)
        .bind(data.errors_on_received() as i64)
        .bind(data.errors_on_transmitted() as i64)
        .bind(packets_received as i64)
        .bind(packets_transmitted as i64)
        .bind(data.total_received() as i64)
        .bind(data.total_transmitted() as i64)
        .bind(data.total_packets_received() as i64)
        .bind(data.total_packets_transmitted() as i64)
        .bind(data.total_errors_on_received() as i64)
        .bind(data.total_errors_on_transmitted() as i64)
        .bind(ip.join(","))
        .bind(chrono::Local::now().timestamp())
        .execute(pool)
        .await;

        GLOBAL_SYSTEM_INFO
            .write()
            .await
            .entry(format!("net_{}_packets_received", interface_name))
            .and_modify(|v| *v = data.packets_received().to_string())
            .or_insert(data.packets_received().to_string());
        GLOBAL_SYSTEM_INFO
            .write()
            .await
            .entry(format!("net_{}_packets_transmitted", interface_name))
            .and_modify(|v| *v = data.packets_transmitted().to_string())
            .or_insert(data.packets_transmitted().to_string());
        GLOBAL_SYSTEM_INFO
            .write()
            .await
            .entry(format!("net_{}_total_received", interface_name))
            .and_modify(|v| *v = data.total_received().to_string())
            .or_insert(data.total_received().to_string());
        GLOBAL_SYSTEM_INFO
            .write()
            .await
            .entry(format!("net_{}_total_transmitted", interface_name))
            .and_modify(|v| *v = data.total_transmitted().to_string())
            .or_insert(data.total_transmitted().to_string());
    }

    let _ = sqlx::query(
        "insert into system_stats (loadavg_one,loadavg_five,loadavg_fifteen,cpu_usage,memory_usage,swap_usage,created_at) values ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(load_avg.one)
    .bind(load_avg.five)
    .bind(load_avg.fifteen)
    .bind(cpu_usage)
    .bind(memory_usage)
    .bind(swap_usage)
    .bind(chrono::Local::now().timestamp())
    .execute(pool)
    .await;
}

pub async fn init_system_jobs() {
    let sched: JobScheduler = JobScheduler::new()
        .await
        .expect("can't start job scheduler");
    add_jobs(&sched).await;

    if sched.start().await.is_err() {
        info!("scheduled start failed")
    }
    let mut sched_map = GLOBAL_SCHEDULED_MAP.write().await;
    sched_map.insert("zap".to_string(), sched);
}

pub async fn add_jobs(sched: &JobScheduler) {
    let job = Job::new_async("1/10 * * * * *", |_uuid, _lock| {
        Box::pin(system_scheduled_task())
    })
    .unwrap();
    let system_job_uuid = sched.add(job).await.unwrap();
    let mut job_map = GLOBAL_JOB_MAP.write().await;
    job_map.insert("system".to_string(), system_job_uuid.into());
}

pub async fn stop_system_job() {
    let mut zap_sched_map = GLOBAL_SCHEDULED_MAP.write().await;
    if let Some(sched) = zap_sched_map.get_mut("zap") {
        let _ = sched.shutdown().await;
    }
    zap_sched_map.clear();
    GLOBAL_JOB_MAP.write().await.clear();
}

pub async fn start_system_job() {
    let mut zap_sched_map = GLOBAL_SCHEDULED_MAP.write().await;
    let sched: JobScheduler = JobScheduler::new()
        .await
        .expect("can't start job scheduler");
    add_jobs(&sched).await;
    let _ = sched.start().await;
    zap_sched_map.insert("zap".to_string(), sched);
}
