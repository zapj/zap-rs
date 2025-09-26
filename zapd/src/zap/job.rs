use std::collections::HashMap;

use lazy_static::lazy_static;
use tokio::sync::RwLock;
use tokio_cron_scheduler::{job::job_data::Uuid, Job, JobScheduler};
use tracing::info;


lazy_static!{
    static ref GLOBAL_SCHEDULED_MAP : RwLock<HashMap<String,JobScheduler>> = RwLock::new(HashMap::new());
    static ref GLOBAL_JOB_MAP : RwLock<HashMap<String,Uuid>> = RwLock::new(HashMap::new());
}

async fn system_scheduled_task() {
    println!("Scheduled task executed at: {:?}", chrono::Utc::now());
    // Your task logic here, e.g., database operations, API calls
}


pub async fn init_system_jobs() {
    let sched: JobScheduler = JobScheduler::new().await.expect("can't start job scheduler");
    add_jobs(&sched).await;
    
    if let Err(_) = sched.start().await {
        info!("scheduled start failed")
    }
    let mut sched_map = GLOBAL_SCHEDULED_MAP.write().await;
    sched.start().await;
    sched_map.insert("zap".to_string(), sched);
    
    
}

pub async fn add_jobs(sched : &JobScheduler) {
    let job = Job::new_async("1/10 * * * * *", |_uuid, _lock| Box::pin(system_scheduled_task())).unwrap();
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
    let sched: JobScheduler = JobScheduler::new().await.expect("can't start job scheduler");
    add_jobs(&sched).await;
    let _ = sched.start().await;
    zap_sched_map.insert("zap".to_string(), sched);
}