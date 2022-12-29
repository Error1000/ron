use crate::{Mutex, primitives::{LazyInitialised, MutexGuard}, program::Program, UART};
use alloc::vec::Vec;

static TASK_LIST: Mutex<LazyInitialised<Vec<Option<Program>>>> = Mutex::from(LazyInitialised::uninit());

// Because you can't otherwise create tasks during a tick, which a task might want to do if it, for eg., forks
static NUMBER_OF_TASKS: Mutex<LazyInitialised<usize>> = Mutex::from(LazyInitialised::uninit());
static NEW_TASK_LIST: Mutex<LazyInitialised<Vec<Option<Program>>>> = Mutex::from(LazyInitialised::uninit());

pub fn init() {
    TASK_LIST.lock().set(Vec::new());
    NEW_TASK_LIST.lock().set(Vec::new());
    NUMBER_OF_TASKS.lock().set(0);
}

// Returns: The new processes pid
pub fn new_task(mut program:  Program) -> usize {
    let mut list = NEW_TASK_LIST.lock();
    let new_pid = NUMBER_OF_TASKS.lock().clone();
    program.data.pid = Some(new_pid);
    list.push(Some(program));
    **NUMBER_OF_TASKS.lock() += 1;
    new_pid
}

fn move_new_tasks_into_list(list: &mut MutexGuard<LazyInitialised<Vec<Option<Program>>>>) {
    let mut new_list = NEW_TASK_LIST.lock();
    while let Some(p) = new_list.pop() { list.push(p); }
    **NUMBER_OF_TASKS.lock() = list.len();
}

// Forcibly removes process from task list, calling it's destructor which deallocates all it's resources
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
        if let Some(program) = &mut list[i] {
            if program.tick().is_none() {
                use core::fmt::Write;
                writeln!(UART.lock(), "State after program ended: {:?}", program).unwrap();
                list[i] = None;
                // Drain None's if it wouldn't affect the indices of elements that are Some
                while list.last().map(|val|val.is_none()).unwrap_or(false) { list.pop(); }
            }
            
            move_new_tasks_into_list(&mut list);
        }
    }

    return list.len() > 0;
}