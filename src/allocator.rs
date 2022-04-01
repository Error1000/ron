use core::{alloc::GlobalAlloc, ptr::{self, null_mut}};
use crate::primitives::Mutex;
use core::fmt::Debug;



#[global_allocator]
pub static ALLOCATOR: Mutex<BasicAlloc> = Mutex::from(BasicAlloc::new());

pub struct BasicAlloc{
    base: *mut u8,
    len: usize,
    alloc_count: usize,
    next: usize,
    stashed_deallocations: [(*mut u8, core::alloc::Layout); 1024]
}

impl Debug for BasicAlloc{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BasicAlloc").field("base", &self.base).field("len", &self.len).field("alloc_count", &self.alloc_count).field("next", &self.next).field("stashed_deallocations", &self.stashed_deallocations).finish()
    }
}
impl BasicAlloc{
    const fn new() -> Self{
        Self{
            base: ptr::null_mut(),
            len: 0,
            alloc_count: 0,
            next: 0,
            stashed_deallocations: [(null_mut(), core::alloc::Layout::new::<u8>()); 1024]
        }
    }

    pub fn init(&mut self, base: *mut u8, len: usize) {
        self.base = base;
        self.len = len;
    }

    pub fn get_heap_size(&self) -> usize { self.next }

    pub fn find_free_ind(&self) -> Option<usize> {
        for (i, e) in self.stashed_deallocations.iter().enumerate(){
            if e.0 == null_mut() { return Some(i); }
        }
        return None;
    }

    fn try_dealloc(&mut self, ptr: *mut u8, layout: core::alloc::Layout) -> bool {
        let mut did_dealloc = false;
         // If we are asked to deallocate from the top of the stack we can do that :)
         if unsafe{ ptr.add(layout.size()).sub(self.next) } == self.base{ // (ptr = alloc.base) alloc.base+alloc.size-head = base <=> alloc.base+alloc.size = base+head <=> we are asked to deallocate from the top of the stack
            self.next -= layout.size();
            did_dealloc = true;
        }

        // Or if we have deallocated all the allocations
        if self.alloc_count == 0 { 
            self.next = 0; 
            for e in self.stashed_deallocations.iter_mut(){
                *e = (null_mut(), core::alloc::Layout::new::<u8>());
            }
            did_dealloc = true; 
        }
        // Otherwise we can't deallocate
        did_dealloc
    }
}


unsafe impl GlobalAlloc for Mutex<BasicAlloc>{
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let mut s = self.lock();
        let extra = s.next % layout.align(); // align can never be 0 if it is constructed correctly
        if extra != 0 { 
            if let Ok(padding) = core::alloc::Layout::from_size_align(layout.align()-extra, 1){ 
                s.next += padding.size();
                if s.next > s.len { return null_mut(); }
                let padding_ptr = s.base.add(s.next).add(padding.size());
                if let Some(ind) = s.find_free_ind(){
                    s.stashed_deallocations[ind] = (padding_ptr, padding);
                }else{
                    // Just leak memory idk ¯\_(ツ)_/¯
                }
            }else{
                return null_mut();
            }
        }
        s.next += layout.size();
        if s.next >= s.len { return null_mut(); } // OOM :^(
        s.alloc_count += 1;
        let ret_ptr = s.base.add(s.next).sub(layout.size());    
        ret_ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let mut s = self.lock();
        s.alloc_count -= 1; // Keeps track if we have gotten the same amount of deallocations as allocations,
                            // so we can reset everything that we leaked in that case

        let did_dealloc = s.try_dealloc(ptr, layout);
        
        if !did_dealloc{
            // Store failed deallocation in some free spot in array
            if let Some(i) = s.find_free_ind(){
                s.stashed_deallocations[i] = (ptr, layout);
            }else{
                // Just leak memory idk ¯\_(ツ)_/¯
            }
        }else{ // Maybe the deallocation we just did allows us to deallocate even more
            // NOTE: sort_by allocates, so don't use it
            // Sort array by allocations that are closest to the top of the stack ( a.k.a ascending order, lowest addresses first )
            s.stashed_deallocations.sort_unstable_by(|alloc1, alloc2|alloc2.0.cmp(&alloc1.0) );
            
            for i in 0..s.stashed_deallocations.len(){
                let stashed_dealloc = s.stashed_deallocations[i];
                if stashed_dealloc.0 == null_mut() { break; } // If the failed allocation with the lowest address is null, then obv. all other allocations are null as well (a.k.a there are no more allocations to consider)
                if s.try_dealloc(stashed_dealloc.0, stashed_dealloc.1) {
                    s.stashed_deallocations[i] = (null_mut(), core::alloc::Layout::new::<u8>());
                }else{
                    // If we can't deallocate the allocation with the lowest address, there is no point in trying the others because they will be above it
                     break; 
                }
            }
        }
    }
}
