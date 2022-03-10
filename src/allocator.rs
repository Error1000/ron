use core::{alloc::GlobalAlloc, ptr::{self, null_mut}};
use crate::primitives::Mutex;




#[global_allocator]
pub static ALLOCATOR: Mutex<BasicAlloc> = Mutex::from(BasicAlloc::new());

pub struct BasicAlloc{
    base: *mut u8,
    len: usize,
    alloc_count: usize,
    next: usize,
    failed_deallocations: [(*mut u8, core::alloc::Layout); 1024]
}


impl BasicAlloc{
    const fn new() -> Self{
        Self{
            base: ptr::null_mut(),
            len: 0,
            alloc_count: 0,
            next: 0,
            failed_deallocations: [(null_mut(), core::alloc::Layout::new::<u8>()); 1024]
        }
    }

    pub fn init(&mut self, base: *mut u8, len: usize) {
        self.base = base;
        self.len = len;
    }

    pub fn get_heap_size(&self) -> usize { self.next }
    
    fn try_dealloc(&mut self, ptr: *mut u8, layout: core::alloc::Layout) -> bool {
        let mut did_dealloc = false;
         // If we are asked to deallocate from the top of the stack we can do that :)
         if unsafe{ ptr.add(layout.size()).sub(self.next) } == self.base{
            self.next -= layout.size();
            let extra = self.next % layout.align(); // Number of bytes over the biggest multiple of layout.align() that is less than self.next
            if extra != 0 { self.next -= layout.align() - extra; }
            did_dealloc = true;
        }
        if self.alloc_count == 0 { 
            self.next = 0; 
            for e in self.failed_deallocations.iter_mut(){
                *e = (null_mut(), core::alloc::Layout::new::<u8>());
            }
            did_dealloc = true; 
        }
        did_dealloc
    }
}


unsafe impl GlobalAlloc for Mutex<BasicAlloc>{
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let mut s = self.lock();
        let extra = s.next % layout.align(); // Number of bytes over the biggest multiple of layout.align() that is less than self.next
        if extra != 0 { s.next += layout.align() - extra; }
        s.next += layout.size();
        if s.next > s.len { return ptr::null_mut(); } // OOM :^(
        s.alloc_count += 1;
        let ret_ptr = s.base.add(s.next).sub(layout.size());    
        ret_ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let mut s = self.lock();
        s.alloc_count -= 1; // Keeps track if we have gotten the same amount of deallocations as allocations,
                            // so we can reset everything that we leaked in that case

        let did_dealloc = s.try_dealloc(ptr, layout);
        
        let find_free_ind = || -> Option<usize> {
            for (i, e) in s.failed_deallocations.iter().enumerate(){
                if e.0 == null_mut() { return Some(i); }
            }
            return None;
        };
        if !did_dealloc{
            // Store failed deallocation in some free spot in array
            if let Some(i) = find_free_ind(){
                s.failed_deallocations[i] = (ptr, layout);
            }else{
                // Just leak memory idk ¯\_(ツ)_/¯
            }
        }else{
            // NOTE: sort_by allocates, so don't use it
            // Sort array by allocations that are closest to the top of the stack ( a.k.a descending order, biggest addresses first )
            s.failed_deallocations.sort_unstable_by(|alloc1, alloc2|alloc2.0.cmp(&alloc1.0) );
            
            // Maybe this deallocation now allows us to deallocate even more
            for i in 0..s.failed_deallocations.len(){
                let failed_alloc = s.failed_deallocations[i];
                if failed_alloc.0 == null_mut() { break; } // If the allocation with the biggest address is null, then obv. all other allocations are null as well (a.k.a there are no more allocations to consider)
                if s.try_dealloc(failed_alloc.0, failed_alloc.1) {
                    s.failed_deallocations[i] = (null_mut(), core::alloc::Layout::new::<u8>());
                }else{ break; }
            }
        }
    }
}
