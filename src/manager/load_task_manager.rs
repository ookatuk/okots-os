use alloc::sync::Arc;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::SeqCst;
use spin::RwLock;
use spin::Once;
use x86_64::instructions::interrupts::without_interrupts;
use crate::MAX_DO_ITEM;

#[derive(Debug, Default)]
pub struct LoadTaskManager {
    pub ended_task: AtomicUsize,
    pub do_parent: Once<Arc<RwLock<f64>>>,
}

impl LoadTaskManager {
    pub fn get_add_func(self: Arc<Self>) -> Arc<impl Fn() + 'static> {
        Arc::new(move || {
            self.ended_task.fetch_add(1, SeqCst);
            without_interrupts(|| {
                if let Some(parent) = self.do_parent.get() {
                    *parent.write() = self.ended_task.load(SeqCst) as f64 / MAX_DO_ITEM as f64;
                }
            });
        })
    }

    pub fn add(self: Arc<Self>) {
        self.get_add_func()();
    }
}