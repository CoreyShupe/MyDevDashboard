//! `tasks` feature — business logic, composed of one service per part (AGENTS.md §2).
//!
//! `TasksService` is a thin aggregate: `backend.tasks.stage`, `.ticket`, `.note`.

pub mod note;
pub mod stage;
pub mod ticket;

use sqlx::postgres::PgPool;

use note::NoteService;
use stage::StageService;
use ticket::TicketService;

/// Aggregate of the tasks feature's part-services, sharing one pool.
#[derive(Clone)]
pub struct TasksService {
    pub stage: StageService,
    pub ticket: TicketService,
    pub note: NoteService,
}

impl TasksService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            stage: StageService::new(pool.clone()),
            ticket: TicketService::new(pool.clone()),
            note: NoteService::new(pool),
        }
    }
}
