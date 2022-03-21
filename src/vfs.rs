use core::{convert::TryFrom, ops::Deref, cell::RefCell, fmt::Display};

use alloc::{string::String, vec::Vec, rc::Rc, borrow::ToOwned};

use crate::{primitives::{Mutex, LazyInitialised}};

pub static VFS_ROOT: Mutex<LazyInitialised<Rc<RefCell<VFSNode>>>> = Mutex::from(LazyInitialised::uninit());



pub trait INode{
    fn get_name(&self) -> String;
}

pub trait IFolder: INode{
    fn get_children(&self) -> Vec<Node>;
}

pub trait IFile: INode{
    fn read(&self, offset: usize, len: usize) -> Option<Vec<u8>>;
    fn write(&mut self, offset: usize, data: &[u8]);
}

#[derive(Clone)]
pub enum Node{
    File(Rc<RefCell<dyn IFile>>),
    Folder(Rc<RefCell<dyn IFolder>>)
}
impl INode for Node{
    fn get_name(&self) -> String {
        match self{
            Node::File(f) => (*f).borrow().get_name(),
            Node::Folder(f) => (*f).borrow().get_name()
        }
    }
}

#[derive(Clone)]
pub struct Path{
    inner: String
}

impl Path{
    pub fn root() -> Self{
        Self{inner: String::from("/")}
    }

    pub fn last(&self) -> &str{
        self.inner.split("/").last().expect("Path should be valid at all times!")
    }

    pub fn del_last(&mut self){
        loop{
            if let Some(c) = self.inner.pop(){
                if c == '/' { if self.inner.len() == 0 { self.inner.push('/'); } break; }
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

impl Display for Path{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl Deref for Path{
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl PartialEq for Path{
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl TryFrom<&str> for Path{
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if !value.starts_with("/") { return Err(());}
        if !value.contains("/") { return Err(()); }
        Ok(Path{inner: String::from(value)})
    }
}

#[derive(Clone)]
pub struct VFSNode{
    path: Path,
    parent: Option<Rc<RefCell<VFSNode>>>,
    children: Vec<Rc<RefCell<VFSNode>>>,
    pub mountpoint: Option<Rc<RefCell<dyn IFolder>>>
}

impl VFSNode{
    pub fn new_root() -> Self{
        Self{
            path: Path::root(),
            parent: None,
            children: Vec::new(),
            mountpoint: None
        }
    }

    pub fn new_folder(slf: Rc<RefCell<VFSNode>>, name: &str) -> Rc<RefCell<VFSNode>>{
        let mut new_p = (*slf).borrow().path.clone();
        new_p.append(name);
        let new_f = Rc::new(RefCell::new(Self{
            path: new_p,
            parent: Some(slf.clone()),
            children: Vec::new(),
            mountpoint: None,
        }));
        (*slf).borrow_mut().children.push(new_f.clone());
        new_f
    }

    pub fn del_folder(slf: Rc<RefCell<VFSNode>>, name: &str) -> bool{
        let mut di = None;
        for (i, c) in (*slf).borrow().children.iter().enumerate(){
            if (**c).borrow().get_children().len() != 0 { continue; }
            if (**c).borrow().path.last() == name{
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
            if (**c).borrow().path.last() == name{
                return Some(c.clone());
            }
        }
        None
    }

    pub fn get_parent(&self) -> Option<&RefCell<VFSNode>>{
        self.parent.as_deref()
    }

    pub fn get_path(&self) -> &Path{
        &self.path
    }
}

impl INode for VFSNode{
    fn get_name(&self) -> String {
        self.path.last().to_owned()
    }
}

impl IFolder for VFSNode{
    fn get_children(&self) -> Vec<Node>{
        let mut v = Vec::<Node>::new();
        if let Some(mnt) = &self.mountpoint{
            for c in (*mnt).borrow().get_children(){
                v.push(c.clone());
            }
        }
        for c in &self.children{
            // Name resolution
            if v.iter().any(|child| child.get_name() == (*c).borrow().get_name()){
                continue;
            }
            v.push(Node::Folder(c.clone()));
        }
        v
    }
}