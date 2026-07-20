//! System layer: owns the database pool and all business logic, sliced by feature.
//!
//! MUST NOT import `egui`/`eframe` or anything in `ui/`. The UI reaches this layer only
//! through the `app/` bridge. See AGENTS.md §2.

pub mod db;
pub mod profile;
pub mod tasks;

use sqlx::postgres::PgPool;

use profile::ProfileService;
use tasks::TasksService;

/// Aggregate of every feature's service, sharing a single connection pool.
///
/// Feature handlers receive `&Backend`, which is what makes cross-feature reach possible
/// (e.g. the `profile` handler seeding the `tasks` board on first onboarding).
#[derive(Clone)]
pub struct Backend {
    pub profile: ProfileService,
    pub tasks: TasksService,
}

impl Backend {
    pub fn new(pool: PgPool) -> Self {
        Self {
            profile: ProfileService::new(pool.clone()),
            tasks: TasksService::new(pool),
        }
    }
}
