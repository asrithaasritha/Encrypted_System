use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

use chrono::{DateTime, Duration, Local};

#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub next_run: DateTime<Local>,
    pub interval: Duration, // how often to repeat
    pub action: JobAction,
}

#[derive(Debug, Clone)]
pub enum JobAction {
    CheckReminders,
    AutoCreateFromDueDates,
}

// Internal wrapper for heap ordering
#[derive(Debug, Clone, Eq, PartialEq)]
struct ScheduledJob {
    next_run_secs: i64, // Unix timestamp
    job: Job,
}

impl Ord for ScheduledJob {
    fn cmp(&self, other: &Self) -> Ordering {
        self.next_run_secs.cmp(&other.next_run_secs)
    }
}

impl PartialOrd for ScheduledJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Equality based on job ID
impl Eq for Job {}

impl PartialEq for Job {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

pub struct Scheduler {
    queue: BinaryHeap<Reverse<ScheduledJob>>, // min-heap via Reverse
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
        }
    }

    pub fn add_job(&mut self, job: Job) {
        let secs = job.next_run.timestamp();

        self.queue.push(Reverse(ScheduledJob {
            next_run_secs: secs,
            job,
        }));
    }

    /// Returns jobs that are due right now (next_run <= now)
    pub fn pop_due(&mut self) -> Vec<Job> {
        let now = Local::now().timestamp();
        let mut due = Vec::new();

        while let Some(Reverse(sj)) = self.queue.peek() {
            if sj.next_run_secs <= now {
                let Reverse(sj) = self.queue.pop().unwrap();
                due.push(sj.job);
            } else {
                break;
            }
        }

        due
    }

    /// After a job runs, reschedule it
    pub fn reschedule(&mut self, mut job: Job) {
        job.next_run = Local::now() + job.interval;
        self.add_job(job);
    }

    /// Time until next job (in seconds)
    pub fn next_run_in_secs(&self) -> Option<i64> {
        self.queue.peek().map(|Reverse(sj)| {
            let diff = sj.next_run_secs - Local::now().timestamp();
            diff.max(0)
        })
    }
}