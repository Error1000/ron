use crate::{primitives::Mutex, UART};
use core::alloc::Allocator;
use core::fmt::Debug;
use core::ptr::NonNull;
use core::{
    alloc::GlobalAlloc,
    ptr::{self, null_mut},
};

#[global_allocator]
pub static ALLOCATOR: Mutex<BasicAlloc> = Mutex::from(BasicAlloc::new(false));

pub static PROGRAM_ALLOCATOR: ProgramBasicAlloc = ProgramBasicAlloc(Mutex::from(BasicAlloc::new(false)));

pub struct ProgramBasicAlloc(pub Mutex<BasicAlloc>);

// This is a bump allocator that doesn't leak as much memory as a normal bump allocator
#[derive(Clone)]
pub struct BasicAlloc {
    base: *mut u8,
    len: usize,
    alloc_count: usize,
    next: usize,
    is_virtual: bool, // Tells the allocator that the pointers are not real, they are virtual and should not be dereferenced
    stashed_deallocations: [(*mut u8, core::alloc::Layout); 1024],
}

impl Debug for BasicAlloc {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BasicAlloc")
            .field("base", &self.base)
            .field("len", &self.len)
            .field("alloc_count", &self.alloc_count)
            .field("next", &self.next)
            .field("stashed_deallocations (len)", &self.stashed_deallocations.iter().filter(|val| val.0 != null_mut()).count())
            .finish()
    }
}

impl BasicAlloc {
    const fn new(is_virtual: bool) -> Self {
        Self {
            base: ptr::null_mut(),
            len: 0,
            alloc_count: 0,
            next: 0,
            is_virtual,
            stashed_deallocations: [(null_mut(), core::alloc::Layout::new::<u8>()); 1024],
        }
    }

    pub fn from(base: *mut u8, len: usize, is_virtual: bool) -> Self {
        Self {
            base,
            len,
            alloc_count: 0,
            next: 0,
            is_virtual,
            stashed_deallocations: [(null_mut(), core::alloc::Layout::new::<u8>()); 1024],
        }
    }

    pub fn init(&mut self, base: *mut u8, len: usize) -> Option<()> {
        if self.alloc_count != 0 {
            return None;
        }
        self.base = base;
        self.len = len;
        return Some(());
    }

    pub fn grow_heap_space(&mut self, len: usize) {
        self.len += len;
    }

    pub fn get_heap_used(&self) -> usize {
        self.next
    }
    pub fn get_heap_max(&self) -> usize {
        self.len
    }

    pub fn find_free_dealloc_ind(&self) -> Option<usize> {
        for (i, e) in self.stashed_deallocations.iter().enumerate() {
            if e.0 == null_mut() {
                return Some(i);
            }
        }
        return None;
    }

    fn try_dealloc(&mut self, ptr: *mut u8, layout: core::alloc::Layout) -> bool {
        // If we are asked to deallocate from the top of the stack we can do that :)
        if unsafe { ptr.add(layout.size()).sub(self.next) } == self.base {
            // (ptr = alloc.base) alloc.base+alloc.size-head = base <=> alloc.base+alloc.size = base+head <=> we are asked to deallocate from the top of the stack
            self.next -= layout.size();
            return true;
        }

        // Or if we have deallocated all the allocations
        if self.alloc_count == 0 {
            self.next = 0;
            for e in self.stashed_deallocations.iter_mut() {
                *e = (null_mut(), core::alloc::Layout::new::<u8>());
            }
            return true;
        }

        // Otherwise we can't deallocate
        return false;
    }

    pub fn alloc(&mut self, layout: core::alloc::Layout) -> *mut u8 {
        if self.next % layout.align() != 0 {
            // If we are not aligned
            if let Ok(padding) = core::alloc::Layout::from_size_align(
                layout.align()
                    - (self.next % layout.align()/* no div by 0 because align() can't return zero if the layout is constructed correctly */),
                1,
            ) {
                self.next += padding.size();
                if self.next >= self.len {
                    self.next -= padding.size();
                    return null_mut();
                } // OOM :^(
                  // Note: since we never call dealloc() on this padding allocation explicitly there is no need to inc alloc_count
                let padding_ptr = unsafe { self.base.add(self.next).sub(padding.size()) };
                if let Some(ind) = self.find_free_dealloc_ind() {
                    self.stashed_deallocations[ind] = (padding_ptr, padding);
                } else {
                    use core::fmt::Write;
                    let _ = writeln!(UART.lock(), "Leaking memory :)");
                    // Just leak memory idk ¯\_(ツ)_/¯
                }
            } else {
                return null_mut();
            }
        }

        self.next += layout.size();
        if self.next >= self.len {
            self.next -= layout.size();
            return null_mut();
        } // OOM :^(
        self.alloc_count += 1;
        let ret_ptr = unsafe { self.base.add(self.next).sub(layout.size()) };
        ret_ptr
    }

    pub fn dealloc(&mut self, ptr: *mut u8, layout: core::alloc::Layout) {
        self.alloc_count -= 1; // Keeps track if we have gotten the same amount of deallocations as allocations,
                               // so we can reset everything that we leaked in that case

        let did_dealloc = self.try_dealloc(ptr, layout);

        if !did_dealloc {
            // Store failed deallocation in some free spot in array
            if let Some(i) = self.find_free_dealloc_ind() {
                self.stashed_deallocations[i] = (ptr, layout);
            } else {
                use core::fmt::Write;
                let _ = writeln!(UART.lock(), "Leaking memory :)");
                // Just leak memory idk ¯\_(ツ)_/¯
            }
        } else {
            // Maybe the deallocation we just did allows us to deallocate even more
            // NOTE: sort_by allocates, so don't use it
            // Sort array by allocations that are closest to the top of the stack ( a.k.a descending order, highest addresses first because we grow the allocator's stack by adding to the base address )
            self.stashed_deallocations.sort_unstable_by(|alloc1, alloc2| alloc2.0.cmp(&alloc1.0));

            for i in 0..self.stashed_deallocations.len() {
                let stashed_dealloc = self.stashed_deallocations[i];
                if stashed_dealloc.0 == null_mut() {
                    break;
                } // If the failed deallocation with the highest address is null, then obv. all other deallocations are null as well (a.k.a there are no more deallocations to consider)
                if self.try_dealloc(stashed_dealloc.0, stashed_dealloc.1) {
                    self.stashed_deallocations[i] = (null_mut(), core::alloc::Layout::new::<u8>());
                } else {
                    // If we can't deallocate the allocation with the highest address, there is no point in trying the others because they will be under it
                    break;
                }
            }
        }
    }


    pub unsafe fn realloc(&mut self, ptr: *mut u8, layout: core::alloc::Layout, new_size: usize) -> *mut u8 {
        let Ok(new_layout) = core::alloc::Layout::from_size_align(new_size, layout.align()) else {
            return null_mut();
        };

        // TODO: Ability to reuse old allocation when shrinking
        let new_ptr = self.alloc(new_layout);
        if !new_ptr.is_null() {
            // If we could allocate a new block
                if !self.is_virtual{
                    rlibc::mem::memcpy(new_ptr as *mut i8, ptr as *mut i8, core::cmp::min(layout.size(), new_size));
                }
                self.dealloc(ptr, layout);
        }
        new_ptr
    }
}

unsafe impl GlobalAlloc for Mutex<BasicAlloc> {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let mut s = self.lock();
        s.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let mut s = self.lock();
        s.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: core::alloc::Layout, new_size: usize) -> *mut u8 {
        let mut s = self.lock();
        s.realloc(ptr, layout, new_size)
    }
}

unsafe impl Allocator for &ProgramBasicAlloc {
    fn allocate(&self, layout: core::alloc::Layout) -> Result<NonNull<[u8]>, core::alloc::AllocError> {
        let mut s = self.0.lock();

        NonNull::<[u8]>::new(unsafe { core::slice::from_raw_parts(s.alloc(layout), layout.size()) } as *const [u8] as *mut [u8])
            .ok_or(core::alloc::AllocError)
    }

    unsafe fn deallocate(&self, ptr: ptr::NonNull<u8>, layout: core::alloc::Layout) {
        let mut s = self.0.lock();
        s.dealloc(ptr.as_ptr(), layout)
    }
}
