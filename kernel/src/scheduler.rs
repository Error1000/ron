use crate::{Mutex, primitives::{LazyInitialised, MutexGuard}, process::{Process, ProcessState, WaitInformation, ProcessPipe}, UART};
use alloc::vec::Vec;

static TASK_LIST: Mutex<LazyInitialised<Vec<Option<Process>>>> = Mutex::from(LazyInitialised::uninit());

// Because you can't otherwise create tasks during a tick, which a task might want to do if it, for eg., forks
static NUMBER_OF_TASKS: Mutex<LazyInitialised<usize>> = Mutex::from(LazyInitialised::uninit());
static NEW_TASK_LIST: Mutex<LazyInitialised<Vec<Option<Process>>>> = Mutex::from(LazyInitialised::uninit());

pub static PIPES: Mutex<LazyInitialised<Vec<Option<ProcessPipe>>>> = Mutex::from(LazyInitialised::uninit());


// WARNING: Global allocator must be initialized before calling this function!
pub fn init() {
    TASK_LIST.lock().set(Vec::new());
    NEW_TASK_LIST.lock().set(Vec::new());
    NUMBER_OF_TASKS.lock().set(0);
    PIPES.lock().set(Vec::new());
}

// Returns: The new processes pid
pub fn new_task(mut process:  Process) -> usize {
    let mut list = NEW_TASK_LIST.lock();
    let new_pid = NUMBER_OF_TASKS.lock().clone();
    process.data.pid = Some(new_pid);
    list.push(Some(process));
    **NUMBER_OF_TASKS.lock() += 1;
    new_pid
}

fn move_new_tasks_into_list(list: &mut MutexGuard<LazyInitialised<Vec<Option<Process>>>>) {
    let mut new_list = NEW_TASK_LIST.lock();
    while let Some(p) = new_list.pop() { list.push(p); }
    **NUMBER_OF_TASKS.lock() = list.len();
}


// Forcibly removes process from task list, calling it's destructor which deallocates all it's resources
// NOTE: This skips the process state TERMINATED_WAITING_TO_BE_DEALLOCATED and just removes the process directly
pub fn kill_task(pid: usize) {
    let mut list = TASK_LIST.lock();
    move_new_tasks_into_list(&mut list); // Since we have a lock might as well make sure we have all the tasks in one list
    list[pid] = None;

    // Drain None's if it wouldn't affect the indices of elements that are Some
    while list.last().map(|val|val.is_none()).unwrap_or(false) { list.pop(); }
}

pub fn tick() -> bool {
    let mut list = TASK_LIST.lock();
    move_new_tasks_into_list(&mut list); // Since we have a lock might as well make sure we have all the tasks in one list

    for i in 0..list.len() {
        if list[i].is_none() { continue; }
        // SAFTEY: list[i].unwrap() is guaranteed to work because of the if above

        match list[i].as_ref().unwrap().data.state {
            ProcessState::RUNNING | crate::process::ProcessState::RUNNING_NEW_CHILD_JUST_FORKED  | ProcessState::FINISHED_WAITING_FOR_CHILD_PROCESS(_) => {
                let process = list[i].as_mut().unwrap();
                if process.tick().is_none() { // Program ended due to illegal instruction or another type of exception
                    // FIXME: Should use SIGILL when i get around to implementing signals
                    if process.data.parent_pid != None { // If we are a child we don't want to be deallocated until the parent acknowledges us
                        process.data.state = ProcessState::TERMINATED_DUE_TO_ILLEGAL_INSTRUCTION_WAITING_TO_BE_DEALLOCATED;
                    } else {
                        process.data.state = ProcessState::TERMINATED_DUE_TO_ILLEGAL_INSTRUCTION_CHILD_WAITING_FOR_PARENT_ACKNOWLEDGEMENT;
                    }
                }
                    
                move_new_tasks_into_list(&mut list); // In case the process called a syscall which created a new process like fork, move the new process into the list
            }

            ProcessState::WAITING_FOR_CHILD_PROCESS{cpid} => {
                // A state change is considered to be:
                // the child terminated; the child was stopped by a signal; or
                // the child was resumed by a signal.
                // Source: man waitpid 
                let our_pid = list[i].as_ref().unwrap().data.pid.unwrap();

                let mut state_change_of_child = None;
                let mut waiting_is_invalid = false;
                
                if let Some(cpid) = cpid { // We are waiting for a specific process
                    if let Some(Some(child)) = list.get_mut(cpid) {
                        if child.data.parent_pid.unwrap() == our_pid {
                            if let ProcessState::TERMINATED_NORMALLY_CHILD_WAITING_FOR_PARENT_ACKNOWLEDGEMENT{exit_code} = child.data.state {
                                state_change_of_child = Some(WaitInformation{cpid: child.data.pid.unwrap(), exit_code});
                                child.data.state = ProcessState::TERMINATED_NORMALLY_WAITING_TO_BE_DEALLOCATED{exit_code};
                            }
                        }else{
                            waiting_is_invalid = true;
                        }
                    }else{
                        waiting_is_invalid = true;
                    }
                } else { // We are waiting for any child process
                    waiting_is_invalid = true;
                    for p in list.iter_mut().filter_map(|val|val.as_mut()) {
                        // Search for procceses that are children waiting, and their parent is us
                        if p.data.parent_pid == Some(our_pid) {
                            waiting_is_invalid = false;
                            if let ProcessState::TERMINATED_NORMALLY_CHILD_WAITING_FOR_PARENT_ACKNOWLEDGEMENT{exit_code} = p.data.state {
                                state_change_of_child =  Some(WaitInformation{cpid: p.data.pid.unwrap(), exit_code});
                                p.data.state = ProcessState::TERMINATED_NORMALLY_WAITING_TO_BE_DEALLOCATED{exit_code};
                                break;
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
                    writeln!(UART.lock(), "Child with pid: {} has been orphaned!", list[i].as_ref().unwrap().data.pid.unwrap()).unwrap();
                    // For now just switch to TERMINATED_NORMALLY_WAITING_TO_BE_DEALLOCATED
                    list[i].as_mut().unwrap().data.state = ProcessState::TERMINATED_NORMALLY_WAITING_TO_BE_DEALLOCATED{exit_code};
                }
            }

            ProcessState::TERMINATED_DUE_TO_ILLEGAL_INSTRUCTION_CHILD_WAITING_FOR_PARENT_ACKNOWLEDGEMENT => {
                // Check to see if we have been orphaned
                let parents_pid = list[i].as_ref().unwrap().data.parent_pid.unwrap();
                let parent_exists = list.get_mut(parents_pid).map(|p|p.is_some()).unwrap_or(false);
                if !parent_exists {
                    // FIXME: Implement adoption properly
                    use core::fmt::Write;
                    writeln!(UART.lock(), "Child with pid: {} has been orphaned!", list[i].as_ref().unwrap().data.pid.unwrap()).unwrap();
                    // For now just switch to TERMINATED_DUE_TO_ILLEGAL_INSTRUCTION_WAITING_TO_BE_DEALLOCATED
                    list[i].as_mut().unwrap().data.state = ProcessState::TERMINATED_DUE_TO_ILLEGAL_INSTRUCTION_WAITING_TO_BE_DEALLOCATED;
                }
            }

            ProcessState::TERMINATED_NORMALLY_WAITING_TO_BE_DEALLOCATED{exit_code: _} => {
                use core::fmt::Write;
                writeln!(UART.lock(), "State after process ended normally: {:?}", list[i].as_ref().unwrap()).unwrap();
                list[i] = None;
                // Drain None's if it wouldn't affect the indices of elements that are Some
                while list.last().map(|val|val.is_none()).unwrap_or(false) { list.pop(); }
            }

            ProcessState::TERMINATED_DUE_TO_ILLEGAL_INSTRUCTION_WAITING_TO_BE_DEALLOCATED => {
                use core::fmt::Write;
                writeln!(UART.lock(), "State after process ended due to illegal instruction: {:?}", list[i].as_ref().unwrap()).unwrap();
                list[i] = None;
                // Drain None's if it wouldn't affect the indices of elements that are Some
                while list.last().map(|val|val.is_none()).unwrap_or(false) { list.pop(); }
            }
        }
        
    }

    return list.len() > 0;
}