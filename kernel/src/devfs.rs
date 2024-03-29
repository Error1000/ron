use core::cell::RefCell;

use alloc::{rc::Rc, string::String, vec::Vec};

use crate::vfs::{self, IFile, Node};

pub struct DevFS {
    disk_devices: Vec<(String, Rc<RefCell<dyn IFile>>)>,
}

impl DevFS {
    pub fn new() -> Self {
        Self { disk_devices: Vec::new() }
    }

    pub fn add_device_file(&mut self, dev: Rc<RefCell<dyn IFile>>, name: String) {
        self.disk_devices.push((name, dev))
    }
}

impl vfs::IFolder for DevFS {
    fn get_children(&self) -> Vec<(String, Node)> {
        let mut v = Vec::<(String, Node)>::new();
        for c in &self.disk_devices {
            v.push((c.0.clone(), Node::File(c.1.clone())))
        }
        v
    }

    fn create_empty_child(&mut self, _name: &str, _typ: vfs::NodeType) -> Option<Node> {
        None
    }

    fn unlink_or_delete_empty_child(&mut self, _name: &str) -> Option<()> {
        None
    }
}
