use crate::{Mutex, primitives::{LazyInitialised, MutexGuard}, process::{Process, ProcessState, WaitInformation, ProcessPipe, ProcessSignal, WaitAction}, UART};
use alloc::{vec::Vec, collections::VecDeque};
use rlibc::sys::SignalType;

static TASK_LIST: Mutex<LazyInitialised<Vec<Option<Process>>>> = Mutex::from(LazyInitialised::uninit());

// Because you can't otherwise create tasks during a tick, which a task might want to do if it, for eg., forks
static NUMBER_OF_TASKS: Mutex<LazyInitialised<usize>> = Mutex::from(LazyInitialised::uninit());
static NEW_TASK_LIST: Mutex<LazyInitialised<Vec<Option<Process>>>> = Mutex::from(LazyInitialised::uninit());

pub static PIPES: Mutex<LazyInitialised<Vec<Option<ProcessPipe>>>> = Mutex::from(LazyInitialised::uninit());

// A global list of queues of signals for processes
pub static SIGNAL_QUEUES: Mutex<LazyInitialised<Vec<Option<VecDeque<ProcessSignal>>>>> = Mutex::from(LazyInitialised::uninit());


// WARNING: Global allocator must be initialized before calling this function!
pub fn init() {
    TASK_LIST.lock().set(Vec::new());
    NEW_TASK_LIST.lock().set(Vec::new());
    NUMBER_OF_TASKS.lock().set(0);
    PIPES.lock().set(Vec::new());
    SIGNAL_QUEUES.lock().set(Vec::new());
}

// Returns: The new processes pid
pub fn new_task(mut process:  Process) -> usize {
    let mut list = NEW_TASK_LIST.lock();
    let new_pid = NUMBER_OF_TASKS.lock().clone()+1;
    process.data.pid = Some(new_pid);
    list.push(Some(process));
    SIGNAL_QUEUES.lock().push(Some(VecDeque::new())); // Setup a signal queue for the process
    **NUMBER_OF_TASKS.lock() += 1;
    new_pid
}

fn move_new_tasks_into_list(list: &mut MutexGuard<LazyInitialised<Vec<Option<Process>>>>) {
    let mut new_list = NEW_TASK_LIST.lock();
    while let Some(p) = new_list.pop() { list.push(p); }
    **NUMBER_OF_TASKS.lock() = list.len();
}


// Queues a signal to be received by the program on the next tick
// Returns None if pid is invalid
pub fn kill_task(pid: usize, signal: ProcessSignal) -> Option<()>{
    let mut signals = SIGNAL_QUEUES.lock();
    // NOTE: If the pid exists then it must have a signal queue, so therefore if we can not find a signal queue then the pid is invalid
    let signal_queue = signals.get_mut(pid-1)?.as_mut()?;
    signal_queue.push_back(signal);
    Some(())
}


pub fn tick() -> bool {
    let mut list = TASK_LIST.lock();
    move_new_tasks_into_list(&mut list); // Since we have a lock might as well make sure we have all the tasks in one list
    for i in 0..list.len() {
        if list[i].is_none() { continue; }
        // SAFTEY: list[i].unwrap() is guaranteed to work because of the if above
        // Handle queued signals of process list[i]
        {
            let mut signals = SIGNAL_QUEUES.lock();
            let signal_queue = signals[i].as_mut().unwrap();
            while let Some(signal) = signal_queue.pop_front() {
                list[i].as_mut().unwrap().recive_signal(signal);
            }
        }

        match list[i].as_ref().unwrap().data.state {
            ProcessState::RUNNING | crate::process::ProcessState::RUNNING_NEW_CHILD_JUST_FORKED  | ProcessState::FINISHED_WAITING_FOR_CHILD_PROCESS(_) => {
                if list[i].as_mut().unwrap().tick().is_none() { // Program ended due to illegal instruction or another type of exception
                    kill_task(i+1, ProcessSignal { signal_type: SignalType::SIGILL });
                }
                    
                move_new_tasks_into_list(&mut list); // In case the process called a syscall which created a new process like fork, move the new process into the list
            }

            ProcessState::WAITING_FOR_CHILD_PROCESS{cpid} => {
                // A state change is considered to be:
                // the child terminated; the child was stopped by a signal; or
                // the child was resumed by a signal.

                // By default, waitpid() waits only for terminated children, but this behavior is modifiable via the options argument, as described below. 
                // Source: man waitpid 

                let our_pid = i+1;

                let mut state_change_of_child = None;
                let mut waiting_is_invalid = false;
                
                if let Some(cpid) = cpid { // We are waiting for a specific process
                    if let Some(Some(child)) = list.get_mut(cpid-1) {
                        if child.data.parent_pid.unwrap() == our_pid {
                            match child.data.state {
                                ProcessState::TERMINATED_NORMALLY_CHILD_WAITING_FOR_PARENT_ACKNOWLEDGEMENT { exit_code } => {
                                    state_change_of_child = Some(WaitInformation{cpid: child.data.pid.unwrap(), action: WaitAction::EXITED { exit_code }});
                                    child.data.state = ProcessState::TERMINATED_NORMALLY_WAITING_TO_BE_DEALLOCATED{exit_code};
                                }

                                ProcessState::TERMINATED_DUE_TO_SIGNAL_CHILD_WAITING_FOR_PARENT_ACKNOWLEDGEMENT { signal } => {
                                    state_change_of_child = Some(WaitInformation{cpid: child.data.pid.unwrap(), action: WaitAction::TERMINATED_BY_SIGNAL { signal }});
                                    child.data.state = ProcessState::TERMINATED_DUE_TO_SIGNAL_WAITING_TO_BE_DEALLOCATED { signal };
                                }
                                
                                _ => ()
                            }
                        }else{
                            waiting_is_invalid = true;
                        }
                    }else{
                        waiting_is_invalid = true;
                    }
                } else { // We are waiting for any child process
                    waiting_is_invalid = true;
                    for proc in list.iter_mut().filter_map(|val|val.as_mut()) {
                        // Search for procceses that are children waiting, and their parent is us
                        if proc.data.parent_pid == Some(our_pid) {
                            waiting_is_invalid = false;
                            match proc.data.state {
                                ProcessState::TERMINATED_NORMALLY_CHILD_WAITING_FOR_PARENT_ACKNOWLEDGEMENT { exit_code } => {
                                    state_change_of_child = Some(WaitInformation{cpid: proc.data.pid.unwrap(), action: WaitAction::EXITED { exit_code }});
                                    proc.data.state = ProcessState::TERMINATED_NORMALLY_WAITING_TO_BE_DEALLOCATED{exit_code};
                                    break;
                                }

                                ProcessState::TERMINATED_DUE_TO_SIGNAL_CHILD_WAITING_FOR_PARENT_ACKNOWLEDGEMENT { signal } => {
                                    state_change_of_child = Some(WaitInformation{cpid: proc.data.pid.unwrap(), action: WaitAction::TERMINATED_BY_SIGNAL { signal }});
                                    proc.data.state = ProcessState::TERMINATED_DUE_TO_SIGNAL_WAITING_TO_BE_DEALLOCATED { signal };
                                    break;
                                }
                                
                                _ => ()
                            }
                        }
                    }
                    // If there were no processes whose parents is us then we know that there is nobody to wait for
                    // So that is why we start with waiting_is_invalid = true;
                }
                  
                if let Some(info) = state_change_of_child {
                    list[i].as_mut().unwrap().data.state = ProcessState::FINISHED_WAITING_FOR_CHILD_PROCESS(Some(info));
                }else if waiting_is_invalid {
                    list[i].as_mut().unwrap().data.state = ProcessState::FINISHED_WAITING_FOR_CHILD_PROCESS(None);
                }else{
                    // We still need to wait more
                }
            }

            ProcessState::WAITING_FOR_READ_PIPE { pipe_index } => {
                // Wait for pipe to get data
                let pipes = PIPES.lock();
                let pipe = pipes[pipe_index].as_ref().unwrap();
                if pipe.buf.len() > 0 || pipe.writers_count == 0 {
                    list[i].as_mut().unwrap().data.state = ProcessState::RUNNING;
                }
            }

            ProcessState::TERMINATED_NORMALLY_CHILD_WAITING_FOR_PARENT_ACKNOWLEDGEMENT{exit_code} => {             
                // Check to see if we have been orphaned
                let parents_pid = list[i].as_ref().unwrap().data.parent_pid.unwrap();
                let parent_exists = list.get_mut(parents_pid).map(|p|p.is_some()).unwrap_or(false);
                if !parent_exists {
                    // FIXME: Implement adoption properly
                    use core::fmt::Write;
                    writeln!(UART.lock(), "Orphan child with pid: {} has terminated normally!", i+1).unwrap();
                    // For now just switch to TERMINATED_NORMALLY_WAITING_TO_BE_DEALLOCATED
                    list[i].as_mut().unwrap().data.state = ProcessState::TERMINATED_NORMALLY_WAITING_TO_BE_DEALLOCATED{exit_code};
                }
            }

            ProcessState::TERMINATED_DUE_TO_SIGNAL_CHILD_WAITING_FOR_PARENT_ACKNOWLEDGEMENT { signal } => {
                // Check to see if we have been orphaned
                let parents_pid = list[i].as_ref().unwrap().data.parent_pid.unwrap();
                let parent_exists = list.get_mut(parents_pid).map(|p|p.is_some()).unwrap_or(false);
                if !parent_exists {
                    // FIXME: Implement adoption properly
                    use core::fmt::Write;
                    writeln!(UART.lock(), "Orphan child with pid: {} has terminated due to signal: {:?}!", i+1, signal).unwrap();
                    // For now just switch to TERMINATED_DUE_TO_SIGNAL_WAITING_TO_BE_DEALLOCATED
                    list[i].as_mut().unwrap().data.state = ProcessState::TERMINATED_DUE_TO_SIGNAL_WAITING_TO_BE_DEALLOCATED{signal};
                }
            }


            ProcessState::TERMINATED_NORMALLY_WAITING_TO_BE_DEALLOCATED{exit_code} => {
                use core::fmt::Write;
                writeln!(UART.lock(), "State after process ended normally with code 0x{:x}: {:?}", exit_code, list[i].as_ref().unwrap()).unwrap();
                list[i] = None;

                // Drain None's if it wouldn't affect the indices of elements that are Some
                while list.last().map(|val|val.is_none()).unwrap_or(false) { list.pop(); }
                list.shrink_to_fit();

                // Remove our signal queue
                let mut signals = SIGNAL_QUEUES.lock();
                signals[i] = None;

                // Drain None's if it wouldn't affect the indices of elements that are Some
                while signals.last().map(|val|val.is_none()).unwrap_or(false) { signals.pop(); }
                signals.shrink_to_fit();
            }

            ProcessState::TERMINATED_DUE_TO_SIGNAL_WAITING_TO_BE_DEALLOCATED { signal } => {
                use core::fmt::Write;
                writeln!(UART.lock(), "State after process ended due to signal {:?}: {:?}", signal, list[i].as_ref().unwrap()).unwrap();
                list[i] = None;

                // Drain None's if it wouldn't affect the indices of elements that are Some
                while list.last().map(|val|val.is_none()).unwrap_or(false) { list.pop(); }
                list.shrink_to_fit();

                // Remove our signal queue
                let mut signals = SIGNAL_QUEUES.lock();
                signals[i] = None;

                // Drain None's if it wouldn't affect the indices of elements that are Some
                while signals.last().map(|val|val.is_none()).unwrap_or(false) { signals.pop(); }
                signals.shrink_to_fit();
            }
        }
        
    }

    
    return list.len() > 0;
}