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
    TaskEventRecord
};

// rewite
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

    // 2. Spawn child. std::process::Child's Drop does not reap, so the
    //    kernel keeps the process reachable until our own wait4pid below.
    let child_pid = Command::new("/bin/sleep").arg("1").spawn()?.id() as c_int;
    eprintln!("[spawn] child_pid={child_pid} /bin/sleep 1");

    // 3. PollFd array over the N rings.
    let mut pfds: Vec<PollFd> = rings.iter()
        .map(|r| PollFd { fd: r.as_raw_fd(), events: POLLIN, revents: 0 })
        .collect();

    // 4. Event loop until we see the child's EXIT record.
    let mut child_exited = false;
    while !child_exited {
        let n_ready = poll_fds(&mut pfds, 2_500)?;
        if n_ready == 0 {
            eprintln!("[poll] timeout");
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
                            eprintln!("[{t:>5}ms cpu{i}] FORK  pid={} ppid={}", r.pid, r.ppid);
                        }
                    }
                    PERF_RECORD_EXIT => {
                        if let Some(r) = TaskEventRecord::try_from_bytes(bytes) {
                            eprintln!("[{t:>5}ms cpu{i}] EXIT  pid={} ppid={}", r.pid, r.ppid);
                            if r.pid as c_int == child_pid { child_exited = true; }
                        }
                    }
                    PERF_RECORD_COMM => {
                        eprintln!("[{t:>5}ms cpu{i}] COMM  (process renamed / exec'd)");
                    }
                    _ => { /* ignore LOST, THROTTLE, MMAP etc */ }
                }
            });
        }
    }

    // 5. Reap via wait4 — demonstrates the raw FFI we'd use in runtime.
    let status = wait4pid(child_pid, 0)?;

    if w_if_exited(status) {
        eprintln!("[reap] pid={child_pid} exited normally, code={}", w_exit_status(status));
    } else if w_if_signaled(status) {
        eprintln!("[reap] pid={child_pid} killed by signal {}", w_term_sig(status));
    } else {
        eprintln!("[reap] pid={child_pid} raw status=0x{status:x}");
    }

    Ok(())
}
