//! One background operation and one replaceable pending operation. Superseded
//! results never escape the worker, including after cancellation.
use anyhow::Result;
use std::sync::{Arc, Condvar, Mutex};

type Work<T> = Box<dyn FnOnce() -> Result<T> + Send>;
struct State<T> {
    generation: u64,
    pending: Option<(u64, Work<T>)>,
    completed: Option<(u64, Result<T>)>,
    stopped: bool,
}
pub struct LatestWorker<T> {
    shared: Arc<(Mutex<State<T>>, Condvar)>,
}
impl<T: Send + 'static> LatestWorker<T> {
    pub fn new(wake: impl Fn() + Send + 'static) -> Result<Self> {
        let shared = Arc::new((
            Mutex::new(State {
                generation: 0,
                pending: None,
                completed: None,
                stopped: false,
            }),
            Condvar::new(),
        ));
        let worker = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("image-processing".into())
            .spawn(move || {
                loop {
                    let (generation, work) = {
                        let (lock, changed) = &*worker;
                        let mut state = lock.lock().unwrap();
                        while state.pending.is_none() && !state.stopped {
                            state = changed.wait(state).unwrap();
                        }
                        if state.stopped {
                            return;
                        }
                        state.pending.take().unwrap()
                    };
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                        .unwrap_or_else(|_| {
                            Err(anyhow::anyhow!("Image processing worker panicked"))
                        });
                    let mut state = worker.0.lock().unwrap();
                    if !state.stopped && state.generation == generation {
                        state.completed = Some((generation, result));
                        drop(state);
                        wake();
                    }
                }
            })?;
        Ok(Self { shared })
    }
    pub fn submit(&self, work: impl FnOnce() -> Result<T> + Send + 'static) -> u64 {
        let mut state = self.shared.0.lock().unwrap();
        state.generation = state.generation.wrapping_add(1);
        let generation = state.generation;
        state.completed = None;
        state.pending = Some((generation, Box::new(work)));
        self.shared.1.notify_one();
        generation
    }
    pub fn cancel(&self) {
        let mut state = self.shared.0.lock().unwrap();
        state.generation = state.generation.wrapping_add(1);
        state.pending = None;
        state.completed = None;
    }
    pub fn poll(&self, generation: u64) -> Option<Result<T>> {
        let mut state = self.shared.0.lock().unwrap();
        if state
            .completed
            .as_ref()
            .is_some_and(|(id, _)| *id == generation)
        {
            state.completed.take().map(|(_, result)| result)
        } else {
            None
        }
    }
}
impl<T> Drop for LatestWorker<T> {
    fn drop(&mut self) {
        let mut state = self.shared.0.lock().unwrap();
        state.stopped = true;
        state.pending = None;
        state.completed = None;
        self.shared.1.notify_one();
    }
}
