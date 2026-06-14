use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use std::sync::mpsc;
use std::thread;

#[derive(Debug, Clone)]
pub struct MetricSnapshot {
    pub tick: u64,
    pub population: u32,
    pub births: u32,
    pub deaths_starvation: u32,
    pub deaths_old_age: u32,
    pub deaths_predation: u32,
}

pub struct DbWriter {
    sender: mpsc::Sender<MetricSnapshot>,
}

impl DbWriter {
    pub fn new(db_path: &str, run_id: String) -> Self {
        let (tx, rx) = mpsc::channel::<MetricSnapshot>();
        let path = db_path.to_string();

        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build Tokio runtime for DB writer");

            rt.block_on(async move {
                let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path))
                    .unwrap_or_else(|_| SqliteConnectOptions::new().filename(&path))
                    .create_if_missing(true);
                let pool = SqlitePoolOptions::new()
                    .connect_with(options).await
                    .expect("Failed to connect to SQLite DB");
                // Initialize schema
                sqlx::query(
                    "CREATE TABLE IF NOT EXISTS runs (
                        id TEXT PRIMARY KEY,
                        start_time DATETIME DEFAULT CURRENT_TIMESTAMP
                    )"
                ).execute(&pool).await.expect("Failed to create runs table");

                sqlx::query(
                    "CREATE TABLE IF NOT EXISTS metrics (
                        run_id TEXT,
                        tick INTEGER,
                        population INTEGER,
                        births INTEGER,
                        deaths_starvation INTEGER,
                        deaths_old_age INTEGER,
                        deaths_predation INTEGER,
                        FOREIGN KEY(run_id) REFERENCES runs(id)
                    )"
                ).execute(&pool).await.expect("Failed to create metrics table");

                // Insert the new run record
                sqlx::query("INSERT INTO runs (id) VALUES (?)")
                    .bind(&run_id)
                    .execute(&pool).await.expect("Failed to insert run record");
                // Drain the channel
                while let Ok(metric) = rx.recv() {
                    let res = sqlx::query(
                        "INSERT INTO metrics (run_id, tick, population, births, deaths_starvation, deaths_old_age, deaths_predation) 
                         VALUES (?, ?, ?, ?, ?, ?, ?)"
                    )
                    .bind(&run_id)
                    .bind(metric.tick as i64)
                    .bind(metric.population)
                    .bind(metric.births)
                    .bind(metric.deaths_starvation)
                    .bind(metric.deaths_old_age)
                    .bind(metric.deaths_predation)
                    .execute(&pool).await;

                    if let Err(e) = res {
                        tracing::error!("Failed to write metric to DB: {}", e);
                    }}

                tracing::info!("DB writer thread for run {} shutting down", run_id);
            });
        });

        Self { sender: tx }
    }

    pub fn push_metric(&self, metric: MetricSnapshot) {
        let _ = self.sender.send(metric);
    }
}
