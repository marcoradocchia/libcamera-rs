use std::{
    ffi::{CStr, CString},
    io,
    ptr::NonNull,
    sync::Arc,
};

use libcamera_sys::*;

use crate::{camera::Camera, logging::LoggingLevel, utils::handle_result};

pub(crate) struct CameraManagerInner {
    ptr: NonNull<libcamera_camera_manager_t>,
}

impl CameraManagerInner {
    pub(crate) unsafe fn new(ptr: NonNull<libcamera_camera_manager_t>) -> Self {
        Self { ptr }
    }
}

impl Drop for CameraManagerInner {
    fn drop(&mut self) {
        unsafe {
            libcamera_camera_manager_stop(self.ptr.as_ptr());
            libcamera_camera_manager_destroy(self.ptr.as_ptr());
        }
    }
}

unsafe impl Send for CameraManagerInner {}
unsafe impl Sync for CameraManagerInner {}

/// Camera manager used to enumerate available cameras in the system.
pub struct CameraManager {
    inner: Arc<CameraManagerInner>,
}

impl CameraManager {
    /// Initializes `libcamera` and creates [Self].
    pub fn new() -> io::Result<Self> {
        let ptr = NonNull::new(unsafe { libcamera_camera_manager_create() }).unwrap();
        let ret = unsafe { libcamera_camera_manager_start(ptr.as_ptr()) };
        handle_result(ret)?;

        let inner = unsafe { CameraManagerInner::new(ptr) };
        Ok(CameraManager { inner: Arc::new(inner) })
    }

    /// Returns version string of the linked libcamera.
    pub fn version(&self) -> &str {
        unsafe { CStr::from_ptr(libcamera_camera_manager_version(self.inner.ptr.as_ptr())) }
            .to_str()
            .unwrap()
    }

    /// Enumerates cameras within the system.
    pub fn cameras(&self) -> CameraList {
        unsafe {
            CameraList::new(
                NonNull::new(libcamera_camera_manager_cameras(self.inner.ptr.as_ptr())).unwrap(),
                Arc::clone(&self.inner),
            )
        }
    }

    /// Set the log level.
    ///
    /// # Parameters
    ///
    /// * `category` - Free-form category string, a list of those can be seen by running `grep 'LOG_DEFINE_CATEGORY('
    ///   -R` on the `libcamera` source code
    /// * `level` - Maximum log importance level to show, anything more less important than that will be hidden.
    pub fn log_set_level(&self, category: &str, level: LoggingLevel) {
        let category = CString::new(category).expect("category contains null byte");
        let level: &CStr = level.into();
        unsafe {
            libcamera_log_set_level(category.as_ptr(), level.as_ptr());
        }
    }
}

pub struct CameraList {
    ptr: NonNull<libcamera_camera_list_t>,
    inner: Arc<CameraManagerInner>,
}

impl CameraList {
    pub(crate) unsafe fn new(ptr: NonNull<libcamera_camera_list_t>, inner: Arc<CameraManagerInner>) -> Self {
        Self { ptr, inner }
    }

    /// Number of cameras
    pub fn len(&self) -> usize {
        unsafe { libcamera_camera_list_size(self.ptr.as_ptr()) }
    }

    /// Returns `true` if there are no cameras available
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns camera at a given index.
    ///
    /// Returns [None] if index is out of range of available cameras.
    pub fn get(&self, index: usize) -> Option<Camera> {
        let cam_ptr = unsafe { libcamera_camera_list_get(self.ptr.as_ptr(), index as _) };
        NonNull::new(cam_ptr).map(|ptr| unsafe { Camera::new(ptr, Arc::clone(&self.inner)) })
    }

    /// Returns an iterator over the cameras in the list.
    pub fn iter<'d>(&'d self) -> CameraListIter<'d> {
        CameraListIter { list: self, index: 0 }
    }
}

impl Drop for CameraList {
    fn drop(&mut self) {
        unsafe {
            libcamera_camera_list_destroy(self.ptr.as_ptr());
        }
    }
}

pub struct CameraListIter<'d> {
    list: &'d CameraList,
    index: usize,
}

impl<'d> Iterator for CameraListIter<'d> {
    type Item = Camera;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.list.len() {
            let camera = self.list.get(self.index);
            self.index += 1;
            camera
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.list.len().saturating_sub(self.index);
        (len, Some(len))
    }
}

impl<'d> ExactSizeIterator for CameraListIter<'d> {}
