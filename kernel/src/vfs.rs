use core::{
    cell::RefCell,
    convert::TryFrom,
    fmt::{Debug, Display},
    ops::Deref,
};

use alloc::{borrow::ToOwned, rc::Rc, string::String, vec::Vec};

use crate::primitives::{LazyInitialised, Mutex};

pub static VFS_ROOT: Mutex<LazyInitialised<Rc<RefCell<RootFSNode>>>> = Mutex::from(LazyInitialised::uninit());

// Note: This file defines the vfs interface, the vfs indirection and the root fs ( which is basically a ramfs that supports overlay mounting but no files )

pub enum NodeType {
    File,
    Folder,
}

pub trait IFolder {
    fn get_children(&self) -> Vec<(String, Node)>;
    fn create_empty_child(&mut self, name: &str, typ: NodeType) -> Option<Node>;
    fn unlink_or_delete_empty_child(&mut self, name: &str) -> Option<()>;
}

type BytesWritten = Option<usize>;

pub trait IFile {
    fn read(&self, offset: u64, len: usize) -> Option<Vec<u8>>;
    fn write(&mut self, offset: u64, data: &[u8]) -> BytesWritten;
    fn get_size(&self) -> u64;
    fn resize(&mut self, new_size: u64) -> Option<()>;
}

#[derive(Clone)]
pub enum Node {
    File(Rc<RefCell<dyn IFile>>),
    Folder(Rc<RefCell<dyn IFolder>>),
}

impl Node {
    pub fn expect_folder(self) -> Rc<RefCell<dyn IFolder>> {
        match self {
            Node::Folder(f) => f,
            Node::File(_) => panic!("Expected folder, got file!"),
        }
    }

    pub fn expect_file(self) -> Rc<RefCell<dyn IFile>> {
        match self {
            Node::Folder(_) => panic!("Expected file, got folder!"),
            Node::File(f) => f,
        }
    }
}

#[derive(Clone)]
pub struct Path {
    inner: String,
}

impl Path {
    pub fn root() -> Self {
        Self { inner: String::from("/") }
    }

    pub fn last(&self) -> &str {
        self.inner.split("/").last().expect("Path should be valid at all times!")
    }

    pub fn del_last(&mut self) {
        loop {
            if let Some(c) = self.inner.pop() {
                if c == '/' {
                    if self.inner.len() == 0 {
                        self.inner.push('/');
                    }
                    break;
                }
            } else {
                break;
            }
        }
    }

    pub fn push_str(&mut self, subnode: &str) {
        if !self.inner.ends_with("/") {
            self.inner.push('/');
        }
        self.inner.push_str(subnode);
    }

    pub fn get_node(&self) -> Option<Node> {
        let mut cur_node: Node = Node::Folder((**VFS_ROOT.lock()).clone() as Rc<RefCell<dyn IFolder>>);
        let mut cur_path: Path = Path::root();
        let mut nodes = self.inner.split('/');
        'path_traversal_loop: while cur_path != *self {
            let to_find = nodes.next(); // Search for each part of a path, for ex. for the path /test/file, first search for a node named "test" in the root node, then a node named "file" in the "test" node.
            let to_find = if let Some(val) = to_find {
                val
            } else {
                break;
            }
            .trim();
            if to_find == "" {
                continue;
            }

            let children = (*cur_node.clone().expect_folder()).borrow().get_children();
            for (child_name, child_node) in children {
                if child_name == to_find {
                    cur_node = child_node;
                    cur_path.push_str(to_find);
                    continue 'path_traversal_loop;
                }
            }
            return None;
        }

        Some(cur_node)
    }

    pub fn get_rootfs_node(&self) -> Option<Rc<RefCell<RootFSNode>>> {
        let mut to_search: Vec<Rc<RefCell<RootFSNode>>> = Vec::new();
        to_search.push(VFS_ROOT.lock().clone());
        let mut cur_path = Path::root();
        while to_search.len() != 0 {
            if let Some(cur) = to_search.pop() {
                cur_path.push_str(&(*cur).borrow().path.last());
                if cur_path == *self {
                    return Some(cur);
                } else {
                    for c in &(*cur).borrow().children {
                        to_search.push(c.clone());
                    }
                    cur_path.del_last();
                }
            }
        }
        None
    }
}

impl Display for Path {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl Deref for Path {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl TryFrom<&str> for Path {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if !value.starts_with("/") {
            return Err(());
        }
        if !value.contains("/") {
            return Err(());
        }
        Ok(Path { inner: String::from(value) })
    }
}

impl TryFrom<String> for Path {
    type Error = ();

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !value.starts_with("/") {
            return Err(());
        }
        if !value.contains("/") {
            return Err(());
        }
        Ok(Path { inner: String::from(value) })
    }
}

#[derive(Clone)]
pub struct RootFSNode {
    path: Path,
    parent: Option<Rc<RefCell<RootFSNode>>>,
    children: Vec<Rc<RefCell<RootFSNode>>>,
    pub mountpoint: Option<Rc<RefCell<dyn IFolder>>>,
}

impl Debug for RootFSNode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let fmted = self.path.fmt(f);
        f.debug_struct("RootFSNode")
            .field("path", &fmted)
            .field("parent", &self.parent)
            .field("children", &self.children)
            .finish()
    }
}
impl RootFSNode {
    pub fn new_root() -> Self {
        Self { path: Path::root(), parent: None, children: Vec::new(), mountpoint: None }
    }

    pub fn new_folder(slf: Rc<RefCell<RootFSNode>>, name: &str) -> Rc<RefCell<RootFSNode>> {
        let mut new_p = (*slf).borrow().path.clone();
        new_p.push_str(name);
        let new_f =
            Rc::new(RefCell::new(Self { path: new_p, parent: Some(slf.clone()), children: Vec::new(), mountpoint: None }));
        (*slf).borrow_mut().children.push(new_f.clone());
        new_f
    }

    pub fn del_folder(slf: Rc<RefCell<RootFSNode>>, name: &str) -> bool {
        let mut di = None;
        for (i, c) in (*slf).borrow().children.iter().enumerate() {
            if (**c).borrow().get_children().len() != 0 {
                continue;
            }
            if (**c).borrow().path.last() == name {
                di = Some(i);
                break;
            }
        }
        if let Some(i) = di {
            (*slf).borrow_mut().children.remove(i);
            true
        } else {
            false
        }
    }

    pub fn find_folder(slf: Rc<RefCell<RootFSNode>>, name: &str) -> Option<Rc<RefCell<RootFSNode>>> {
        for c in &(*slf).borrow().children {
            if (**c).borrow().path.last() == name {
                return Some(c.clone());
            }
        }
        None
    }

    pub fn get_parent(&self) -> Option<&RefCell<RootFSNode>> {
        self.parent.as_deref()
    }

    pub fn get_path(&self) -> &Path {
        &self.path
    }
}

impl IFolder for RootFSNode {
    // NOTE: Overlays root fs with mountpoint
    fn get_children(&self) -> Vec<(String, Node)> {
        let mut v = Vec::<(String, Node)>::new();
        if let Some(mnt) = &self.mountpoint {
            for c in (**mnt).borrow().get_children() {
                v.push((c.0, c.1.clone()));
            }
        }

        for c in &self.children {
            // Name resolution
            if v.iter().any(|(child_name, _)| *child_name == (**c).borrow().path.last()) {
                continue;
            }
            v.push((c.as_ref().borrow().path.last().to_owned(), Node::Folder(c.clone() as Rc<RefCell<dyn IFolder>>)));
        }
        v
    }

    // Route calls to mountpoint else fail

    fn create_empty_child(&mut self, name: &str, typ: NodeType) -> Option<Node> {
        if let Some(mnt) = &self.mountpoint {
            return (*mnt).borrow_mut().create_empty_child(name, typ);
        } else {
            return None;
        }
    }

    fn unlink_or_delete_empty_child(&mut self, name: &str) -> Option<()> {
        if let Some(mnt) = &mut self.mountpoint {
            return (*mnt).borrow_mut().unlink_or_delete_empty_child(name);
        } else {
            return None;
        }
    }
}
