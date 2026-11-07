use std::collections::HashSet;
use std::ffi::c_int;
use std::io;
use std::process::Command;
use std::thread::available_parallelism;
use std::time::Instant;

use monitoring::{
    poll_fds, wait4pid,
    w_exit_status, w_if_exited, w_if_signaled, w_term_sig,
    PerfCounter, PerfRing, Pid, PollFd,
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

    // 3. PollFd array over the N rings.
    let mut pfds: Vec<PollFd> = rings.iter()
        .map(|r| PollFd { fd: r.as_raw_fd(), events: POLLIN, revents: 0 })
        .collect();

    // 4. Same event loop logic as track_one — only exit condition changed:
    //    keep looping until every direct child has emitted its EXIT record.
    while !pending.is_empty() {
        let n_ready = poll_fds(&mut pfds, 5_000)?;
        if n_ready == 0 {
            eprintln!("[poll] timeout, {} direct child(ren) still alive", pending.len());
            break;
        }

        for (i, pfd) in pfds.iter().enumerate() {
            if pfd.revents & (POLLERR | POLLNVAL) != 0 {
                eprintln!("[poll] ring {i} fd errored (revents={:#x}), skipping", pfd.revents);
                continue;
            }
            if pfd.revents & (POLLIN | POLLHUP) == 0 { continue; }
            rings[i].drain(|hdr, bytes| {
                let t = t0.elapsed().as_millis();
                match hdr.type_ {
                    PERF_RECORD_FORK => {
                        if let Some(r) = TaskEventRecord::try_from_bytes(bytes) {
                            let tag = if pending.contains(&(r.ppid as c_int)) { "GRAND-FORK" } else { "FORK      " };
                            eprintln!("[{t:>5}ms cpu{i}] {tag} pid={} ppid={}", r.pid, r.ppid);
                        }
                    }
                    PERF_RECORD_EXIT => {
                        if let Some(r) = TaskEventRecord::try_from_bytes(bytes) {
                            let tag = if pending.contains(&(r.pid as c_int)) { "EXIT      " } else { "GRAND-EXIT" };
                            eprintln!("[{t:>5}ms cpu{i}] {tag} pid={} ppid={}", r.pid, r.ppid);
                            pending.remove(&(r.pid as c_int));
                        }
                    }
                    PERF_RECORD_COMM => {
                        eprintln!("[{t:>5}ms cpu{i}] COMM       (exec / rename)");
                    }
                    _ => { /* ignore LOST, THROTTLE, MMAP etc */ }
                }
            });
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
