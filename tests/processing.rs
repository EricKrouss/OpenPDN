use openpdn::processing::LatestWorker;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

#[test]
fn pending_operations_coalesce_and_old_results_cannot_escape() {
    let (wake_tx, wake_rx) = mpsc::channel();
    let worker = LatestWorker::new(move || {
        let _ = wake_tx.send(());
    })
    .unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first = worker.submit(move || {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Ok(1)
    });
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let check = Arc::clone(&count);
    let superseded = worker.submit(move || {
        check.fetch_add(1, Ordering::SeqCst);
        Ok(2)
    });
    let latest = worker.submit(|| Ok(3));
    release_tx.send(()).unwrap();
    wake_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(worker.poll(first).is_none());
    assert!(worker.poll(superseded).is_none());
    assert_eq!(worker.poll(latest).unwrap().unwrap(), 3);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert!(worker.poll(latest).is_none());
}
#[test]
fn cancel_discards_in_flight_errors_and_dropping_does_not_wait() {
    let (wake_tx, wake_rx) = mpsc::channel();
    let worker = LatestWorker::new(move || {
        let _ = wake_tx.send(());
    })
    .unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let cancelled = worker.submit(move || {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Err(anyhow::anyhow!("stale error"))
    });
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    worker.cancel();
    let latest = worker.submit(|| Ok(42));
    release_tx.send(()).unwrap();
    wake_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(worker.poll(cancelled).is_none());
    assert_eq!(worker.poll(latest).unwrap().unwrap(), 42);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    worker.submit(move || {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        finished_tx.send(()).unwrap();
        Ok(0)
    });
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(worker); // This must return while the operation is still blocked.
    release_tx.send(()).unwrap();
    finished_rx.recv_timeout(Duration::from_secs(5)).unwrap();
}
#[test]
fn worker_reports_failure_and_processes_a_later_request() {
    let (wake_tx, wake_rx) = mpsc::channel();
    let worker = LatestWorker::new(move || {
        let _ = wake_tx.send(());
    })
    .unwrap();
    let error = worker.submit(|| Err(anyhow::anyhow!("invalid processing parameters")));
    wake_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(worker.poll(error).unwrap().is_err());
    let later = worker.submit(|| Ok(7));
    wake_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(worker.poll(later).unwrap().unwrap(), 7);
}
