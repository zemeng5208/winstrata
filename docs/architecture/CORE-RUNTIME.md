# Core scheduling and event bus

[简体中文](../zh-CN/architecture/CORE-RUNTIME.md) · [Rust library](../../core/src/lib.rs) · [Plan 007](../roadmap/PLAN.md#pr-007)

Plan 007 adds a fixed worker pool and an in-process event bus to `winstrata-core`. `Core::open` does not start either one. Call `Runtime::start` when a task or subscription is needed, and `shutdown` or drop the runtime to join its threads. No async-runtime crate is approved, so this implementation uses the Rust standard library. Replacing that choice requires a separate dependency decision.

## Limits

`Limits::standard` uses 2 workers, task and event queues of 256 and 1,024, a shared event budget of 1,024, batches of 64, 64 subscriptions and a 20 ms admission wait. `Limits::check` rejects zero values and values above the fixed ceilings: 32 workers, 65,536 queued tasks or events, 1,024 subscriptions, a 5 second admission wait, and a batch larger than the subscriber queue. A task timeout must be greater than zero and no more than 24 hours.

These are the minimum protections for this plan. Plan 026 adjusts CPU, memory, and queue budgets inside these ceilings in [Resource budgets](BUDGET.md) and does not change the ceilings. Disk I/O, network, storage, and GUI refresh budgets remain [OI-051](../records/OPEN_ITEMS.md#oi-051). StrataHealth still owns the diagnostic presentation. Plan 025 reads the attached scheduler and bus counters in [Health diagnostics](HEALTH.md). That read does not change these limits.

## Tasks

`Scheduler::submit` takes a priority, a timeout and a closure. A larger priority value runs first. Equal priorities run in submission order. The scheduler keeps one timer thread plus the configured workers. When the task queue is full, submission waits up to the admission limit and then returns `queue_full` without running the task.

`Cancel::cancel` removes a task that has not started and sets the flag seen by `Cancel::is_cancelled`. The timer sets the same flag at the deadline. A running task must poll the flag. The scheduler does not kill a thread that ignores cancellation; that task continues to occupy its worker, and shutdown waits until the closure returns. A panic is counted and the worker continues. Shutdown cancels queued work, signals running work, and joins every scheduler thread.

## Events

`EventBus::publish` takes ownership of a protocol `Event`. The bus validates it, then compares its sequence with the cursor for that source. The first accepted sequence establishes the cursor. A later observation must be the next sequence. A gap marker must cover exactly the open hole, `cursor + 1` through `sequence - 1`. An omitted range on that marker is filled with the hole before delivery. A contiguous marker cannot carry a range. Duplicate and invalid events are returned to the caller with `duplicate_sequence` or `invalid_record`, and a skipped observation is returned with `missing_record`. The bus does not invent a hole before the first sequence and does not invent a gap because an event time moved backward.

Each matching subscriber has a bounded queue. Subscriber queues also share `global_queue`. Higher-priority subscribers take the remaining shared slots first. The hot path stores `Arc<Event>` and does not serialize JSON. `recv_batch` waits up to the caller-supplied limit, which itself cannot exceed 5 seconds, and returns at most the configured batch.

If a matching subscriber cannot accept an event, publish waits up to the admission limit. After that wait it records a loss instead of discarding the event silently. One lost sequence becomes a `lost` gap occupying that sequence and carrying no range. Several consecutive losses become one gap on the last sequence whose range contains the earlier sequences. A type filter records a `filtered` gap in the same shape. Producer gap events still pass a type filter. A device or provider outside the subscription is not a loss for that subscriber. Dropping a subscription releases its unread queue and counts those events as `released`.

The bus is live. A subscriber that joins later does not receive earlier events, and a source identifier does not grant permission. Authenticated transport is the [service IPC](SERVICE-KERNEL.md). A caller that needs history writes the delivered event to the [hybrid store](CORE-STORAGE.md). The bus itself does not write to disk.

## Verification scope

On Linux x86_64, Rust 1.100.0 and an Intel Xeon host, `cargo test --workspace --locked` passed the existing Core and protocol suites plus the runtime tests. A release run moved 20,000 protocol observations from a scheduler task to a subscriber in 39.925 ms, about 501,000 events per second, with contiguous sequences and no losses. Debug runs of the same test also finished within the 10 second bound. The tests cover admission waiting, queue rejection, coalesced loss and filter gaps, cross-batch holes, duplicates, priority, cooperative cancellation, timeout signaling, panic isolation, subscription limits and thread release after shutdown. Linux checks that the joined worker and timer thread IDs no longer carry that scheduler's thread name. The check matches the recorded start time and waits up to two seconds for the kernel to remove the task entry.

Windows execution of this plan, Windows 10/11 client behavior, long-running pressure, GUI latency, dynamic budgets and authenticated IPC were not run. E2E-08 and E2E-15 remain later acceptance scenarios. See the [engineering record](../records/STATUS.md).
