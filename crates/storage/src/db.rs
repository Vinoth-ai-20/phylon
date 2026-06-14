use rusqlite::{params, Connection, Result};
use std::path::Path;

pub enum DbEvent {
    Metrics {
        tick: u64,
        population: u32,
        avg_energy: f32,
        total_food: u32,
    },
    LineageNode {
        entity_id: u64,
        parent_id: Option<u64>,
        generation: u32,
        birth_tick: u64,
        death_tick: Option<u64>,
    },
    DeathUpdate {
        entity_id: u64,
        death_tick: u64,
    },
}

pub struct DbWriter {
    conn: Connection,
}

impl DbWriter {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS metrics (
                tick INTEGER PRIMARY KEY,
                population INTEGER NOT NULL,
                avg_energy REAL NOT NULL,
                total_food INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS lineages (
                entity_id INTEGER PRIMARY KEY,
                parent_id INTEGER,
                generation INTEGER NOT NULL,
                birth_tick INTEGER NOT NULL,
                death_tick INTEGER
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    pub fn get_conn(&self) -> &rusqlite::Connection {
        &self.conn
    }

    pub fn write_event(&mut self, event: DbEvent) -> Result<()> {
        match event {
            DbEvent::Metrics {
                tick,
                population,
                avg_energy,
                total_food,
            } => {
                self.conn.execute(
                    "INSERT OR REPLACE INTO metrics (tick, population, avg_energy, total_food)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![tick as i64, population, avg_energy, total_food],
                )?;
            }
            DbEvent::LineageNode {
                entity_id,
                parent_id,
                generation,
                birth_tick,
                death_tick,
            } => {
                self.conn.execute(
                    "INSERT OR REPLACE INTO lineages (entity_id, parent_id, generation, birth_tick, death_tick)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![entity_id as i64, parent_id.map(|p| p as i64), generation, birth_tick as i64, death_tick.map(|d| d as i64)],
                )?;
            }
            DbEvent::DeathUpdate {
                entity_id,
                death_tick,
            } => {
                self.conn.execute(
                    "UPDATE lineages SET death_tick = ?1 WHERE entity_id = ?2",
                    params![death_tick as i64, entity_id as i64],
                )?;
            }
        }
        Ok(())
    }
}
