use std::collections::HashSet;
use std::ffi::c_int;
use std::io;
use std::process::Command;
use std::thread::available_parallelism;
use std::time::Instant;

use monitoring::{
    wait4pid, Poller,
    w_exit_status, w_if_exited, w_if_signaled, w_term_sig,
    PerfCounter, PerfRing, Pid,
    POLLIN, POLLHUP, POLLERR, POLLNVAL,
    PERF_RECORD_COMM, PERF_RECORD_FORK, PERF_RECORD_EXIT,
    TaskEventRecord,
};

fn main() -> io::Result<()> {
    let t0 = Instant::now();

    // 1. One tracker per online CPU, scoped to self, with inherit.
    let n_cpus = available_parallelism()?.get();
    eprintln!("[init] opening per-CPU trackers across {n_cpus} CPU(s)");

    let mut counters: Vec<PerfCounter> = Vec::with_capacity(n_cpus);
    let mut rings:    Vec<PerfRing>    = Vec::with_capacity(n_cpus);
    for cpu in 0..n_cpus {
        let counter = PerfCounter::open_tree_tracker_cpu(Pid(0), cpu as c_int)?;
        let ring    = counter.attach_ring(4)?;
        counters.push(counter);
        rings.push(ring);
    }

    // 2. Three children with different lifetimes and tree shapes.
    //    The third one is a shell that forks two grandchildren — inherit
    //    should surface those FORK/EXIT records too.
    let specs: Vec<(&str, Vec<&str>)> = vec![
        ("/bin/true",  vec![]),
        ("/bin/sleep", vec!["1"]),
        ("/bin/sh",    vec!["-c", "sleep 0.3 & sleep 0.7 & wait"]),
        ("/bin/sleep",    vec!["20"]),
    ];

    let mut direct_children: Vec<c_int> = Vec::with_capacity(specs.len());
    let mut pending: HashSet<c_int>     = HashSet::new();
    for (prog, args) in &specs {
        let pid = Command::new(prog).args(args).spawn()?.id() as c_int;
        direct_children.push(pid);
        pending.insert(pid);
        eprintln!("[spawn] pid={pid} {prog} {}", args.join(" "));
    }

    // 3. Register each ring with the Poller. Token returned by register()
    //    matches the ring's index in the `rings` Vec by construction.
    let mut poller = Poller::new();
    for ring in &rings {
        poller.register(ring.as_raw_fd());
    }

    // 4. Same event loop logic as track_one — only the dispatch surface
    //    changed from a raw Vec<PollFd> to Poller + Readiness::iter().
    while !pending.is_empty() {
        let ready = poller.wait(5_000)?;
        let mut saw_event = false;
        for (idx, revents) in ready.iter() {
            saw_event = true;
            if revents & (POLLERR | POLLNVAL) != 0 {
                eprintln!("[poll] ring {idx} fd errored (revents={revents:#x}), skipping");
                continue;
            }
            if revents & (POLLIN | POLLHUP) == 0 { continue; }
            rings[idx].drain(|hdr, bytes| {
                let t = t0.elapsed().as_millis();
                match hdr.type_ {
                    PERF_RECORD_FORK => {
                        if let Some(r) = TaskEventRecord::try_from_bytes(bytes) {
                            let tag = if pending.contains(&(r.ppid as c_int)) { "GRAND-FORK" } else { "FORK      " };
                            eprintln!("[{t:>5}ms cpu{idx}] {tag} pid={} ppid={}", r.pid, r.ppid);
                        }
                    }
                    PERF_RECORD_EXIT => {
                        if let Some(r) = TaskEventRecord::try_from_bytes(bytes) {
                            let tag = if pending.contains(&(r.pid as c_int)) { "EXIT      " } else { "GRAND-EXIT" };
                            eprintln!("[{t:>5}ms cpu{idx}] {tag} pid={} ppid={}", r.pid, r.ppid);
                            pending.remove(&(r.pid as c_int));
                        }
                    }
                    PERF_RECORD_COMM => {
                        eprintln!("[{t:>5}ms cpu{idx}] COMM       (exec / rename)");
                    }
                    _ => { /* ignore LOST, THROTTLE, MMAP etc */ }
                }
            });
        }
        if !saw_event {
            eprintln!("[poll] timeout, {} direct child(ren) still alive", pending.len());
            break;
        }
    }

    // 5. Reap each direct child. Grandchildren were already reaped by their
    //    own parent (e.g. the /bin/sh with `wait`) before that parent exited.
    for pid in direct_children {
        let status = wait4pid(pid, 0)?;
        if w_if_exited(status) {
            eprintln!("[reap] pid={pid} exited normally, code={}", w_exit_status(status));
        } else if w_if_signaled(status) {
            eprintln!("[reap] pid={pid} killed by signal {}", w_term_sig(status));
        } else {
            eprintln!("[reap] pid={pid} raw status=0x{status:x}");
        }
    }

    Ok(())
}
