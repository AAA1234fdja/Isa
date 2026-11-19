//! Staged explicit handle-list support for Windows process creation.
//! Inheritable duplicates exist briefly in this process. A concurrent launcher
//! that inherits all handles could still leak them; this is not production-ready.
use std::{
    ffi::c_void,
    io,
    mem::size_of,
    os::windows::io::{AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle, RawHandle},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE},
    System::Threading::{
        DeleteProcThreadAttributeList, GetCurrentProcess, InitializeProcThreadAttributeList,
        UpdateProcThreadAttribute, LPPROC_THREAD_ATTRIBUTE_LIST, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
    },
};

pub(super) fn inheritable_duplicate(source: BorrowedHandle<'_>) -> io::Result<OwnedHandle> {
    // SAFETY: source is borrowed from a caller-owned live handle; pseudo-handle
    // GetCurrentProcess is valid in this process; target points to writable memory.
    let mut target: HANDLE = ptr::null_mut();
    let current = unsafe { GetCurrentProcess() };
    let ok = unsafe {
        DuplicateHandle(
            current,
            source.as_raw_handle() as _,
            current,
            &mut target,
            0,
            1,
            DUPLICATE_SAME_ACCESS,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: DuplicateHandle succeeded and returned a new owned handle.
    Ok(unsafe { OwnedHandle::from_raw_handle(target as RawHandle) })
}

pub(super) struct HandleList<'a> {
    // Keep both allocation and handle list at stable addresses until deletion.
    _storage: Vec<usize>,
    list: LPPROC_THREAD_ATTRIBUTE_LIST,
    _handles: Box<[HANDLE; 3]>,
    _borrow: std::marker::PhantomData<[&'a OwnedHandle; 3]>,
}
impl<'a> HandleList<'a> {
    pub(super) fn new(handles: [&'a OwnedHandle; 3]) -> io::Result<Self> {
        let handles = Box::new(handles.map(|h| h.as_raw_handle() as HANDLE));
        let mut bytes = 0usize;
        // SAFETY: the first call requests only the required allocation size.
        unsafe {
            InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut bytes);
        }
        if bytes == 0 || bytes > 1024 * 1024 {
            return Err(io::Error::other(
                "invalid process attribute allocation size",
            ));
        }
        let words = bytes
            .checked_add(size_of::<usize>() - 1)
            .ok_or_else(|| io::Error::other("attribute size overflow"))?
            / size_of::<usize>();
        let mut storage = vec![0usize; words];
        let list = storage.as_mut_ptr().cast::<c_void>();
        // SAFETY: storage is aligned for pointers and has at least bytes bytes.
        if unsafe { InitializeProcThreadAttributeList(list, 1, 0, &mut bytes) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let result = Self {
            _storage: storage,
            list,
            _handles: handles,
            _borrow: std::marker::PhantomData,
        };
        // SAFETY: the boxed handle array and attribute list remain stable and
        // live through CreateProcessW; all three handles are valid at this point.
        let ok = unsafe {
            UpdateProcThreadAttribute(
                result.list,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                result._handles.as_ptr().cast(),
                size_of::<[HANDLE; 3]>(),
                ptr::null_mut(),
                ptr::null(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(result)
    }
    pub(super) fn as_ptr(&self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.list
    }
}
impl Drop for HandleList<'_> {
    fn drop(&mut self) {
        // SAFETY: list was successfully initialized and is deleted exactly once.
        unsafe {
            DeleteProcThreadAttributeList(self.list);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::io::AsHandle;
    use windows_sys::Win32::Foundation::{GetHandleInformation, HANDLE_FLAG_INHERIT};
    #[test]
    fn windows_process_attributes_duplicate_and_build_handle_list() {
        let (reader, writer) = super::super::windows_pipe::pipe().unwrap();
        let a = inheritable_duplicate(reader.as_handle()).unwrap();
        let b = inheritable_duplicate(writer.as_handle()).unwrap();
        let c = inheritable_duplicate(writer.as_handle()).unwrap();
        for h in [&a, &b, &c] {
            let mut flags = 0;
            assert_ne!(
                unsafe { GetHandleInformation(h.as_raw_handle() as _, &mut flags) },
                0
            );
            assert_ne!(flags & HANDLE_FLAG_INHERIT, 0);
        }
        let list = HandleList::new([&a, &b, &c]).unwrap();
        assert!(!list.as_ptr().is_null());
    }
}
