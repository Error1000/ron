use core::cell::RefCell;

use alloc::{vec::Vec, string::String, rc::Rc, borrow::ToOwned};

use crate::vfs::{self, IFile, Node};

pub struct DevFS{
  disk_devices: Vec<Rc<RefCell<dyn IFile>>>
}

impl DevFS{
  pub fn new() -> Self{
    Self{
      disk_devices: Vec::new()
    }
  }

  pub fn add_device_file(&mut self, dev: Rc<RefCell<dyn IFile>>) {
    self.disk_devices.push(dev)
  }
}

impl vfs::INode for DevFS{
    fn get_name(&self) -> String {
        "dev".to_owned()
    }
}

impl vfs::IFolder for DevFS{
    fn get_children(&self) -> Vec<Node> {
       let mut v = Vec::<Node>::new();
       for c in &self.disk_devices{
         v.push(Node::File(c.clone()))
       }
       v
    }
}