use crate::{
    address::{Address, PhysicalAddress, VirtualAddress},
    vm::phy2virt,
};

/// MIMO Register Read and Write
pub trait MIMORW {
    ///
    /// # Safety
    /// Writes to given Memory Mapped register
    unsafe fn write_reg<T: Sized + Copy>(self, val: T);

    ///
    /// # Safety
    /// Reads from given Memory Mapped Register
    unsafe fn read_reg<T: Sized + Copy>(self) -> T;
}

impl MIMORW for PhysicalAddress {
    unsafe fn write_reg<T: Sized + Copy>(self, val: T) {
        phy2virt(self).write_reg(val);
    }

    unsafe fn read_reg<T: Sized + Copy>(self) -> T {
        phy2virt(self).read_reg()
    }
}

/// Virtual Addresses can be read/written to directly
impl MIMORW for VirtualAddress {
    unsafe fn write_reg<T: Sized + Copy>(self, val: T) {
        core::ptr::write_volatile(self.as_mut_ptr(), val);
    }

    unsafe fn read_reg<T: Sized + Copy>(self) -> T {
        core::ptr::read_volatile(self.as_ptr())
    }
}
