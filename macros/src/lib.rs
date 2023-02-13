#![feature(concat_idents)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, ItemFn};

#[proc_macro_attribute]
pub fn exception_handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let exception_handler = parse_macro_input!(item as ItemFn);
    let exception_handler_func = exception_handler.sig.ident;
    let exception_handler_func_impl = syn::Ident::new(
        &format!("{exception_handler_func}_impl"),
        exception_handler_func.span(),
    );
    let exception_handler_args = exception_handler.sig.inputs;
    let exception_handler_block = exception_handler.block;
    let asm_block = format!(
        r"
        /* 30 general purpose registers + Link Register, ELR_EL1, ESR_EL1, SPSR_EL1 */
        sub sp, sp, #(8 * 34)

        stp x0, x1, [sp]
        stp x2, x3, [sp, #(16 * 1)]
        stp x4, x5, [sp, #(16 * 2)]
        stp x6, x7, [sp, #(16 * 3)]
        stp x8, x9, [sp, #(16 * 4)]
        stp x10, x11, [sp, #(16 * 5)]
        stp x12, x13, [sp, #(16 * 6)]
        stp x14, x15, [sp, #(16 * 7)]
        stp x16, x17, [sp, #(16 * 8)]
        stp x18, x19, [sp, #(16 * 9)]
        stp x20, x21, [sp, #(16 * 10)]
        stp x22, x23, [sp, #(16 * 11)]
        stp x24, x25, [sp, #(16 * 12)]
        stp x26, x27, [sp, #(16 * 13)]
        stp x28, x29, [sp, #(16 * 14)]

        /*
        * Add the exception link register (ELR_EL1), saved program status (SPSR_EL1) and exception
        * syndrome register (ESR_EL1).
        */
        mrs	x1,  ELR_EL1
        mrs	x2,  SPSR_EL1
        mrs	x3,  ESR_EL1

        stp	lr, x1, [sp, #(16 * 15)]
        stp	x2, x3, [sp, #(16 * 16)]

        /* x0 is the first argument for the function called through the handler */
        mov	x0,  sp

        /* Call the handler */
        bl {exception_handler_func_impl}

        ldr	x19,      [sp, #16 * 16]
        ldp	lr,  x20, [sp, #16 * 15]

        msr	SPSR_EL1, x19
        msr	ELR_EL1,  x20

        ldp x0, x1, [sp]
        ldp x2, x3, [sp, #(16 * 1)]
        ldp x4, x5, [sp, #(16 * 2)]
        ldp x6, x7, [sp, #(16 * 3)]
        ldp x8, x9, [sp, #(16 * 4)]
        ldp x10, x11, [sp, #(16 * 5)]
        ldp x12, x13, [sp, #(16 * 6)]
        ldp x14, x15, [sp, #(16 * 7)]
        ldp x16, x17, [sp, #(16 * 8)]
        ldp x18, x19, [sp, #(16 * 9)]
        ldp x20, x21, [sp, #(16 * 10)]
        ldp x22, x23, [sp, #(16 * 11)]
        ldp x24, x25, [sp, #(16 * 12)]
        ldp x26, x27, [sp, #(16 * 13)]
        ldp x28, x29, [sp, #(16 * 14)]
        ldr x30, [sp, #(16 * 15)]

        add sp, sp, #(8 * 34)
        eret",
    );

    let gen = quote! {
        #[no_mangle]
        extern "C" fn #exception_handler_func_impl (#exception_handler_args)
        #exception_handler_block

        #[no_mangle]
        #[naked]
        pub unsafe extern "C" fn #exception_handler_func () {
            core::arch::asm!(#asm_block, options(noreturn));
        }
    };

    gen.into()
}

#[proc_macro_derive(AddressOps)]
pub fn derive_address_ops(item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as DeriveInput);
    let name = &ast.ident;
    let gen = quote! {
        impl Address for #name {
            fn as_raw_ptr(&self) -> usize {
                self.0
            }

            fn as_ptr<T: Sized>(&self) -> *const T {
                self.0 as *const T
            }

            fn as_mut_ptr<T: Sized>(&self) -> *mut T {
                self.0 as *mut T
            }
        }

        impl const core::ops::Add<isize> for #name {
            type Output = Self;

            fn add(self, val: isize) -> Self {
                Self((self.0 as isize + val) as usize)
            }
        }

        impl const core::ops::Sub<isize> for #name {
            type Output = Self;

            fn sub(self, val: isize) -> Self {
                Self((self.0 as isize - val) as usize)
            }
        }

        impl const core::ops::Add<usize> for #name {
            type Output = Self;

            fn add(self, val: usize) -> Self {
                Self(self.0 + val)
            }
        }

        impl const core::ops::Sub<usize> for #name {
            type Output = Self;

            fn sub(self, val: usize) -> Self {
                Self(self.0 - val)
            }
        }

        impl const core::ops::Sub for #name {
            type Output = isize;

            fn sub(self, other: Self) -> isize {
                (self.0 - other.0) as isize
            }
        }

        impl core::ops::AddAssign<isize> for #name {
            fn add_assign(&mut self, val: isize) {
                self.0 = (self.0 as isize + val) as usize;
            }
        }

        impl core::ops::SubAssign<isize> for #name {
            fn sub_assign(&mut self, val: isize) {
                self.0 = (self.0 as isize - val) as usize;
            }
        }

        impl core::ops::AddAssign<usize> for #name {
            fn add_assign(&mut self, val: usize) {
                self.0 = self.0 + val;
            }
        }

        impl core::ops::SubAssign<usize> for #name {
            fn sub_assign(&mut self, val: usize) {
                self.0 = self.0 - val;
            }
        }
    };

    gen.into()
}
