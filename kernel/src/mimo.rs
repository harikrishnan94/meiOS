use crate::{
    address::{Address, PhysicalAddress, VirtualAddress},
    error::Result,
    vm::phy_to_virt,
};

/// MIMO Register Read and Write
pub trait MIMORW {
    ///
    /// # Safety
    /// Writes to given Memory Mapped register
    unsafe fn write_reg<T: Sized + Copy>(self, val: T) -> Result<()>;

    ///
    /// # Safety
    /// Reads from given Memory Mapped Register
    unsafe fn read_reg<T: Sized + Copy>(self) -> Result<T>;
}

impl MIMORW for PhysicalAddress {
    unsafe fn write_reg<T: Sized + Copy>(self, val: T) -> Result<()> {
        phy_to_virt(self)?.write_reg(val)
    }

    unsafe fn read_reg<T: Sized + Copy>(self) -> Result<T> {
        phy_to_virt(self)?.read_reg()
    }
}

/// Virtual Addresses can be read/written to directly
impl MIMORW for VirtualAddress {
    unsafe fn write_reg<T: Sized + Copy>(self, val: T) -> Result<()> {
        core::ptr::write_volatile(self.as_mut_ptr(), val);
        Ok(())
    }

    unsafe fn read_reg<T: Sized + Copy>(self) -> Result<T> {
        Ok(core::ptr::read_volatile(self.as_ptr()))
    }
}
