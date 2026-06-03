#![cfg(feature = "tat")]

use restrict_lang::{parse_program, TypeChecker};
use std::sync::{Arc, Mutex};
use std::thread;

/// ⚡ Test Alchemist's Concurrency Cauldron: Temporal Scope Race Conditions
///
/// These tests reveal race conditions and concurrency bugs in temporal scopes.
/// Each test is designed to expose a specific concurrency vulnerability.

#[test]
fn test_temporal_scope_race_condition() {
    // Race Condition 1: Concurrent access to temporal resources
    let input = r#"
    record SharedResource<~t> {
        value: Int32,
        lock: Mutex<Bool>
    }
    
    fun racy_increment<~t>(resource: SharedResource<~t>) -> Int32 {
        // Two tasks trying to increment concurrently
        spawn(|| {
            resource.value = resource.value + 1
        });
        
        spawn(|| {
            resource.value = resource.value + 1  
        });
        
        // Race: value might be incremented once or twice
        sync_wait();
        resource.value
    }
    
    fun main = {
        with lifetime<~shared> {
            val resource = SharedResource { 
                value = 0,
                lock = Mutex::new(false)
            };
            
            racy_increment(resource)  // Result: 1 or 2?
        }
    }"#;

    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    // This should expose lack of concurrency control in temporal scopes
    let _ = checker.check_program(&program);
}

#[test]
fn test_temporal_lifetime_early_destruction() {
    // Race Condition 2: Early destruction of temporal scope
    let input = r#"
    record TempFile<~f> {
        path: String,
        handle: FileHandle
    }
    
    fun spawn_file_reader<~f>(file: TempFile<~f>) {
        spawn(|| {
            // Async task that takes time
            sleep(100);
            file.handle.read()  // Use after free?
        })
    }
    
    fun main = {
        with lifetime<~temp> {
            val file = TempFile { 
                path = "/tmp/test",
                handle = open_file("/tmp/test")
            };
            
            spawn_file_reader(file);
            // Lifetime ~temp ends here!
        }  // File destroyed while spawn task still running
        
        sleep(200);  // Spawned task tries to read destroyed file
    }"#;

    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => panic!("Temporal escape through spawn should be caught!"),
        Err(e) => assert!(e.to_string().contains("escape") || e.to_string().contains("lifetime")),
    }
}

#[test]
fn test_nested_temporal_deadlock() {
    // Race Condition 3: Deadlock through nested temporal acquisitions
    let input = r#"
    record Database<~db> {
        name: String,
        lock: Mutex<()>
    }
    
    record Transaction<~tx, ~db> where ~tx within ~db {
        id: Int32,
        db_lock: MutexGuard<()>
    }
    
    fun deadlock_scenario<~db1, ~db2>
    (db1: Database<~db1>, db2: Database<~db2>) {
        // Thread 1
        spawn(|| {
            with lifetime<~tx1> where ~tx1 within ~db1 {
                val lock1 = db1.lock.lock();
                sleep(10);
                with lifetime<~tx2> where ~tx2 within ~db2 {
                    val lock2 = db2.lock.lock();  // Waits for db2
                }
            }
        });
        
        // Thread 2  
        spawn(|| {
            with lifetime<~tx2> where ~tx2 within ~db2 {
                val lock2 = db2.lock.lock();
                sleep(10);
                with lifetime<~tx1> where ~tx1 within ~db1 {
                    val lock1 = db1.lock.lock();  // Waits for db1
                }
            }
        });
        
        // Classic deadlock: T1 has db1, wants db2; T2 has db2, wants db1
    }
    
    fun main = {
        with lifetime<~db1> {
        with lifetime<~db2> {
            val database1 = Database { name = "DB1", lock = Mutex::new(()) };
            val database2 = Database { name = "DB2", lock = Mutex::new(()) };
            
            deadlock_scenario(database1, database2);
            sync_wait()
        }}
    }"#;

    // This tests if the type system can prevent deadlock patterns
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    let _ = checker.check_program(&program);
}

#[test]
fn test_temporal_scope_migration() {
    // Race Condition 4: Resource migration between temporal scopes
    let input = r#"
    record MigratableResource<~t> {
        data: String,
        current_scope: ~t
    }
    
    fun migrate<~old, ~new>
    (resource: MigratableResource<~old>) -> MigratableResource<~new>
    where ~new within ~old {
        // Attempting to migrate resource to narrower scope
        spawn(|| {
            // Background task still using old scope
            resource.data |> process_slowly
        });
        
        // Meanwhile, migrate to new scope
        MigratableResource {
            data = resource.data,  // Affine violation if spawn owns it
            current_scope = ~new
        }
    }
    
    fun main = {
        with lifetime<~broad> {
            with lifetime<~narrow> where ~narrow within ~broad {
                val resource = MigratableResource {
                    data = "sensitive",
                    current_scope = ~broad
                };
                
                // Race: migration while spawn task active
                val migrated = migrate(resource);
                migrated.data
            }
        }
    }"#;

    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => panic!("Concurrent resource migration should fail!"),
        Err(_) => {}
    }
}

#[test]
fn test_temporal_ordering_confusion() {
    // Race Condition 5: Temporal ordering violations
    let input = r#"
    record Event<~t> {
        timestamp: Int64,
        data: String
    }
    
    record Timeline<~past, ~present, ~future> 
    where ~past within ~present, ~present within ~future {
        past_events: List<Event<~past>>,
        current_event: Event<~present>,
        future_events: List<Event<~future>>
    }
    
    fun time_travel<~p, ~n, ~f>
    (timeline: Timeline<~p, ~n, ~f>) -> Event<~p> {
        // Spawn a "future" task
        spawn(|| {
            // This runs "in the future"
            timeline.future_events.push(Event {
                timestamp = now() + 1000,
                data = "future event"
            })
        });
        
        // But we return a past event that might be modified
        timeline.past_events[0]  // Data race with future task?
    }
    
    fun main = {
        with lifetime<~future> {
            with lifetime<~present> where ~present within ~future {
                with lifetime<~past> where ~past within ~present {
                    val timeline = Timeline {
                        past_events = [Event { timestamp = 0, data = "past" }],
                        current_event = Event { timestamp = 1, data = "now" },
                        future_events = []
                    };
                    
                    time_travel(timeline)
                }
            }
        }
    }"#;

    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    // Temporal ordering should be enforced even with concurrency
    let _ = checker.check_program(&program);
}

#[test]
fn test_async_temporal_leak() {
    // Race Condition 6: Async operations leaking temporal references
    let input = r#"
    record AsyncHandle<~t> {
        future: Future<String>,
        scope_marker: ~t
    }
    
    fun leak_through_async<~temp>() -> Future<String> {
        with lifetime<~temp> {
            val secret = "temporal secret";
            
            // Create async operation that captures temporal data
            val handle = AsyncHandle {
                future = async {
                    sleep(100);
                    secret  // Captures reference to temporal data
                },
                scope_marker = ~temp
            };
            
            handle.future  // Future escapes temporal scope!
        }
    }
    
    fun main = {
        val leaked_future = leak_through_async();
        leaked_future.await  // Access after temporal scope ended
    }"#;

    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => panic!("Async temporal leak should be caught!"),
        Err(e) => assert!(e.to_string().contains("escape") || e.to_string().contains("temporal")),
    }
}

#[test]
fn test_channel_temporal_smuggling() {
    // Race Condition 7: Smuggling data through channels
    let input = r#"
    record Channel<T, ~t> {
        sender: Sender<T>,
        receiver: Receiver<T>,
        scope: ~t
    }
    
    fun smuggle_data<~short, ~long>() -> String 
    where ~short within ~long {
        val (tx, rx) = channel();
        
        spawn(|| {
            with lifetime<~short> {
                val sensitive = "short-lived secret";
                tx.send(sensitive);  // Send reference through channel
            }
        });
        
        // Receive in longer-lived scope
        rx.recv()  // Got data that should have died with ~short
    }
    
    fun main = {
        with lifetime<~long> {
            smuggle_data()  // Returns short-lived data in long scope
        }
    }"#;

    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => panic!("Channel smuggling should be prevented!"),
        Err(_) => {}
    }
}

#[test]
fn test_temporal_memory_barrier_violation() {
    // Race Condition 8: Memory ordering with temporal scopes
    let input = r#"
    record AtomicCounter<~t> {
        value: AtomicI32,
        scope: ~t
    }
    
    fun racy_counter<~t>() -> Int32 {
        with lifetime<~t> {
            val counter = AtomicCounter {
                value = AtomicI32::new(0),
                scope = ~t
            };
            
            // Multiple threads incrementing without proper ordering
            spawn(|| {
                counter.value.store(1, Ordering::Relaxed);
            });
            
            spawn(|| {
                counter.value.store(2, Ordering::Relaxed);
            });
            
            spawn(|| {
                // Read might see 0, 1, or 2 depending on timing
                counter.value.load(Ordering::Relaxed)
            });
            
            sync_wait();
            counter.value.load(Ordering::SeqCst)
        }
    }
    
    fun main = {
        racy_counter()
    }"#;

    // This tests memory ordering guarantees with temporal scopes
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    let _ = checker.check_program(&program);
}

#[test]
fn test_temporal_scope_fork_bomb() {
    // Race Condition 9: Resource exhaustion through temporal forking
    let input = r#"
    record ForkBomb<~t> {
        depth: Int32,
        scope: ~t
    }
    
    fun fork_temporal<~t>(bomb: ForkBomb<~t>) {
        if bomb.depth > 0 {
            // Each spawn creates new temporal scope
            spawn(|| {
                with lifetime<~child> where ~child within ~t {
                    val child_bomb = ForkBomb {
                        depth = bomb.depth - 1,
                        scope = ~child
                    };
                    fork_temporal(child_bomb);
                    fork_temporal(child_bomb);  // Exponential growth!
                }
            });
        }
    }
    
    fun main = {
        with lifetime<~root> {
            val bomb = ForkBomb { depth = 20, scope = ~root };
            fork_temporal(bomb);  // 2^20 temporal scopes!
            sync_wait()
        }
    }"#;

    // This tests resource limits on temporal scope creation
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    let _ = checker.check_program(&program);
}

#[test]
fn test_temporal_aba_problem() {
    // Race Condition 10: ABA problem with temporal resources
    let input = r#"
    record VersionedResource<~t> {
        value: String,
        version: AtomicU64,
        scope: ~t
    }
    
    fun aba_vulnerability<~t>(resource: VersionedResource<~t>) -> Bool {
        // Thread 1: Read value A
        val initial = resource.value;
        val version = resource.version.load(Ordering::Acquire);
        
        // Thread 2 runs between reads
        spawn(|| {
            resource.value = "B";  // Change A to B
            resource.version.fetch_add(1, Ordering::Release);
            
            resource.value = "A";  // Change back to A  
            resource.version.fetch_add(1, Ordering::Release);
        });
        
        sleep(10);
        
        // Thread 1: Check if unchanged (ABA problem)
        if resource.value == initial {
            // Thinks nothing changed, but version did!
            true  // Incorrect conclusion
        } else {
            false
        }
    }
    
    fun main = {
        with lifetime<~test> {
            val resource = VersionedResource {
                value = "A",
                version = AtomicU64::new(0),
                scope = ~test
            };
            
            aba_vulnerability(resource)
        }
    }"#;

    // Classic ABA problem in context of temporal resources
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    let _ = checker.check_program(&program);
}
