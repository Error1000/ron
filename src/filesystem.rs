use core::{convert::TryFrom, ops::Deref, cell::{RefCell, Ref}};

use alloc::{string::String, vec::Vec, rc::Rc};

use crate::{primitives::{Mutex, LazyInitialised}};

pub static VFS_ROOT: Mutex<LazyInitialised<Rc<RefCell<VFSNode>>>> = Mutex::from(LazyInitialised::uninit());


trait INode{
    type ElementType;
    fn write(&mut self, element: Self::ElementType, offset: usize) -> Option<()>;
    fn read(&self, offset: usize) -> Option<Self::ElementType>;
    fn read_all(&self) -> Option<Vec<Self::ElementType>>;
    fn size(&self) -> usize;
    fn insert(&mut self, element: Self::ElementType, offset: usize) -> Option<()>;
    fn is_readable(&self) -> bool;
    fn is_writeable(&self) -> bool;
    fn is_interactable(&self) -> bool;
}

#[derive(Clone)]
pub struct VFSPath{
    inner: String
}

impl VFSPath{
    pub fn root() -> Self{
        Self{inner: String::from("/")}
    }

    pub fn last(&self) -> Option<&str>{
        self.inner.split("/").last()
    }

    pub fn del_last(&mut self){
        loop{
            if let Some(c) = self.inner.pop(){
                if c == '/' { self.inner.push('/'); break; }
            }else{ break; }
        }
    }

    pub fn append(&mut self, subnode: &str){
        if !self.inner.ends_with("/"){ self.inner.push('/'); }
        self.inner.push_str(subnode);
    }

    pub fn get_node(&self) -> Option<Rc<RefCell<VFSNode>>>{
        let mut to_search: Vec<Rc<RefCell<VFSNode>>> = Vec::new();
        to_search.push(VFS_ROOT.lock().clone());
        while to_search.len() != 0 {
            if let Some(cur) = to_search.pop(){
                if (*cur).borrow().path == *self{
                    return Some(cur);
                }else{
                    for child in &(*cur).borrow().children{
                        to_search.push(child.clone());
                    }
                }
            }
        }
        None
    }
}

impl Deref for VFSPath{
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl PartialEq for VFSPath{
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl TryFrom<&str> for VFSPath{
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if !value.starts_with("/") { return Err(());}
        if !value.contains("/") { return Err(()); }
        Ok(VFSPath{inner: String::from(value)})
    }
}

#[derive(Clone)]
pub struct VFSNode{
    path: VFSPath,
    parent: Option<Rc<RefCell<VFSNode>>>,
    children: Vec<Rc<RefCell<VFSNode>>>,
    mountpoint: Option<VFSPath>
}

impl VFSNode{
    pub fn new_root() -> Self{
        Self{
            path: VFSPath::root(),
            parent: None,
            children: Vec::new(),
            mountpoint: None
        }
    }

    pub fn new_folder(slf: Rc<RefCell<VFSNode>>, name: &str){
        let mut new_p = (*slf).borrow().path.clone();
        new_p.append(name);
        (*slf).borrow_mut().children.push(Rc::new(RefCell::new(Self{
            path: new_p,
            parent: Some(slf.clone()),
            children: Vec::new(),
            mountpoint: None,
        })));
    }

    pub fn del_folder(slf: Rc<RefCell<VFSNode>>, name: &str) -> bool{
        let mut di = None;
        for (i, c) in (*slf).borrow().children.iter().enumerate(){
            if (**c).borrow().get_children().len() != 0 { continue; }
            if (**c).borrow().path.last().expect("Child has well-formed path!") == name{
                di = Some(i);
                break;
            }
        }
        if let Some(i) = di {
            (*slf).borrow_mut().children.remove(i);
            true
        }else{
            false
        }
    }

    pub fn find_folder(slf: Rc<RefCell<VFSNode>>, name: &str) -> Option<Rc<RefCell<VFSNode>>>{
        for c in &(*slf).borrow().children {
            if (**c).borrow().path.last().expect("Child has well-formed path!") == name{
                return Some(c.clone());
            }
        }
        None
    }

    pub fn get_children(&self) -> &Vec<Rc<RefCell<VFSNode>>>{
        &self.children
    }

    pub fn get_parent(&self) -> Option<&RefCell<VFSNode>>{
        self.parent.as_deref()
    }

    pub fn get_path(&self) -> &VFSPath{
        &self.path
    }
}
impl<'a> INode for VFSNode{
    type ElementType = VFSNode;
    fn write(&mut self, element: VFSNode, offset: usize) -> Option<()> {
        if let Some(e) = self.children.get_mut(offset){
            *e = Rc::new(RefCell::new(element));
            Some(())
        }else{
            None
        }
    }

    fn read(&self, offset: usize) -> Option<VFSNode> {
        self.children.get(offset).map(|val|(**val).borrow().clone())
    }

    fn read_all(&self) -> Option<Vec<VFSNode>>{
        Some(self.children.iter().map(|c|(**c).borrow().clone()).collect())
    }

    fn is_readable(&self) -> bool {
        true
    }

    fn is_writeable(&self) -> bool {
        true
    }

    fn is_interactable(&self) -> bool {
        true
    }

    fn size(&self) -> usize {
        self.children.len()
    }

    fn insert(&mut self, element: VFSNode, offset: usize) -> Option<()> {
        self.children.insert(offset, Rc::new(RefCell::new(element)));
        Some(())
    }
}