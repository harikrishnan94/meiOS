#![feature(concat_idents)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, AttributeArgs, DeriveInput, ItemFn};

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

/// Attributes required to mark a function as a constructor. This may be exposed in the future if we determine
/// it to be stable.
#[doc(hidden)]
macro_rules! ctor_attributes {
    () => {
        quote!(
            #[cfg_attr(any(target_os = "none", target_os = "linux", target_os = "android"), link_section = ".init_array")]
            #[cfg_attr(target_os = "freebsd", link_section = ".init_array")]
            #[cfg_attr(target_os = "netbsd", link_section = ".init_array")]
            #[cfg_attr(target_os = "openbsd", link_section = ".init_array")]
            #[cfg_attr(target_os = "dragonfly", link_section = ".init_array")]
            #[cfg_attr(target_os = "illumos", link_section = ".init_array")]
            #[cfg_attr(target_os = "haiku", link_section = ".init_array")]
            #[cfg_attr(any(target_os = "macos", target_os = "ios"), link_section = "__DATA,__mod_init_func")]
            #[cfg_attr(windows, link_section = ".CRT$XCU")]
        )
    };
}

/// Marks a function or static variable as a library/executable constructor.
/// This uses OS-specific linker sections to call a specific function at
/// load time.
///
/// Multiple startup functions/statics are supported, invocation order is not guaranteed.
/// But dependant variables are initialized earlier.
/// Constructor priority is supported (0..=4) and higher priority initializer may not depend
/// on a lower priority initializer (panics on failure). Though, vice-versa is correct.
///
/// # Examples
///
/// Print a startup message (using `libc_print` for safety):
///
/// ```rust
/// # extern crate ctor;
/// # use ctor::*;
/// use libc_print::std_name::println;
///
/// #[ctor]
/// fn foo() {
///   println!("Hello, world!");
/// }
///
/// # fn main() {
/// println!("main()");
/// # }
/// ```
///
/// Make changes to `static` variables:
///
/// ```rust
/// # extern crate ctor;
/// # use ctor::*;
/// # use std::sync::atomic::{AtomicBool, Ordering};
/// static INITED: AtomicBool = AtomicBool::new(false);
///
/// #[ctor]
/// fn foo() {
///   INITED.store(true, Ordering::SeqCst);
/// }
/// ```
///
/// Initialize a `HashMap` at startup time:
///
/// ```rust
/// # extern crate ctor;
/// # use std::collections::HashMap;
/// # use ctor::*;
/// #[ctor]
/// static STATIC_CTOR: HashMap<u32, String> = {
///   let mut m = HashMap::new();
///   for i in 0..100 {
///     m.insert(i, format!("x*100={}", i*100));
///   }
///   m
/// };
///
/// # pub fn main() {
/// #   assert_eq!(STATIC_CTOR.len(), 100);
/// #   assert_eq!(STATIC_CTOR[&20], "x*100=2000");
/// # }
/// ```
///
/// # Details
///
/// The `#[ctor]` macro makes use of linker sections to ensure that a
/// function is run at startup time.
///
/// The above example translates into the following Rust code (approximately):
///
///```rust
/// #[used]
/// #[cfg_attr(any(target_os = "none", target_os = "linux", target_os = "android"), link_section = ".init_array")]
/// #[cfg_attr(target_os = "freebsd", link_section = ".init_array")]
/// #[cfg_attr(target_os = "netbsd", link_section = ".init_array")]
/// #[cfg_attr(target_os = "openbsd", link_section = ".init_array")]
/// #[cfg_attr(target_os = "illumos", link_section = ".init_array")]
/// #[cfg_attr(any(target_os = "macos", target_os = "ios"), link_section = "__DATA,__mod_init_func")]
/// #[cfg_attr(target_os = "windows", link_section = ".CRT$XCU")]
/// static FOO: extern fn() = {
///   #[cfg_attr(any(target_os = "linux", target_os = "android"), link_section = ".init")]
///   extern fn foo() { /* ... */ };
///   foo
/// };
/// ```
#[proc_macro_attribute]
pub fn ctor(args: TokenStream, function: TokenStream) -> TokenStream {
    let item: syn::Item = syn::parse_macro_input!(function);
    let priority = parse_macro_input!(args as AttributeArgs);
    let priority: u8 = if priority.is_empty() {
        0
    } else if priority.len() == 1 {
        let priority = &priority[0];
        match priority {
            syn::NestedMeta::Meta(_) => {
                return quote!(compile_error!("#[ctor] priority must be an integer");).into()
            }
            syn::NestedMeta::Lit(lit) => match lit {
                syn::Lit::Int(ilit) => ilit
                    .base10_parse()
                    .expect("#[ctor] priority must be an integer"),
                _ => return quote!(compile_error!("#[ctor] priority must be an integer");).into(),
            },
        }
    } else {
        return quote!(compile_error!("#[ctor] must have exactly one priority attribute");).into();
    };

    if let syn::Item::Fn(function) = item {
        validate_item("ctor", &function);

        let syn::ItemFn {
            attrs,
            block,
            vis,
            sig:
                syn::Signature {
                    ident,
                    unsafety,
                    constness,
                    abi,
                    ..
                },
            ..
        } = function;

        // Linux/ELF: https://www.exploit-db.com/papers/13234

        let ctor_ident =
            syn::parse_str::<syn::Ident>(format!("{ident}___rust_ctor___ctor").as_ref())
                .expect("Unable to create identifier");

        let tokens = ctor_attributes!();
        let output = quote!(
            #[cfg(not(any(target_os = "none", target_os = "linux", target_os = "android", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd", target_os = "dragonfly", target_os = "illumos", target_os = "haiku", target_os = "macos", target_os = "ios", windows)))]
            compile_error!("#[ctor] is not supported on the current target");

            #(#attrs)*
            #vis #unsafety extern #abi #constness fn #ident() #block

            static_assertions::const_assert!(#priority <= static_init::MAX_INIT_PRIORITY);

            #[used]
            #[allow(non_upper_case_globals)]
            #[doc(hidden)]
            #tokens
            static #ctor_ident
            :
            unsafe extern "C" fn() =
            {
                #[cfg_attr(any(target_os = "none", target_os = "linux", target_os = "android"), link_section = ".init")]
                unsafe extern "C" fn #ctor_ident() {
                    if static_init::CURRENT_INIT_PRIORITY.load(core::sync::atomic::Ordering::Relaxed) == #priority  {
                        #ident();
                    }
                };
                #ctor_ident
            }
            ;
        );

        // eprintln!("{}", output);

        output.into()
    } else if let syn::Item::Static(var) = item {
        let syn::ItemStatic {
            ident,
            mutability,
            expr,
            attrs,
            ty,
            vis,
            ..
        } = var;

        if mutability.is_some() {
            panic!("#[ctor]-annotated static objects must not be mutable");
        }

        if attrs.iter().any(|attr| {
            attr.path
                .segments
                .iter()
                .any(|segment| segment.ident == "no_mangle")
        }) {
            panic!("#[ctor]-annotated static objects do not support #[no_mangle]");
        }

        let ctor_ident =
            syn::parse_str::<syn::Ident>(format!("{ident}___rust_ctor___ctor").as_ref())
                .expect("Unable to create identifier");

        let tokens = ctor_attributes!();
        let output = quote!(
            #[cfg(not(any(target_os = "none", target_os = "linux", target_os = "android", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd", target_os = "dragonfly", target_os = "illumos", target_os = "haiku", target_os = "macos", target_os = "ios", windows)))]
            compile_error!("#[ctor] is not supported on the current target");

            #(#attrs)*
            #vis static #ident: static_init::StaticInitialized<#ty> = static_init::StaticInitialized::new(|| {
                #expr
            },
            #priority);

            static_assertions::const_assert!(#priority <= static_init::MAX_INIT_PRIORITY);

            #[used]
            #[allow(non_upper_case_globals)]
            #tokens
            static #ctor_ident
            :
            unsafe extern "C" fn() = {
                #[cfg_attr(any(target_os = "none", target_os = "linux", target_os = "android"), link_section = ".init")]
                unsafe extern "C" fn initer() {
                    (#ident).init();
                }; initer }
            ;
        );

        // eprintln!("{output}");

        output.into()
    } else {
        panic!("#[ctor] items must be functions or static globals");
    }
}

fn validate_item(typ: &str, item: &syn::ItemFn) {
    let syn::ItemFn { vis, sig, .. } = item;

    // Ensure that visibility modifier is not present
    match vis {
        syn::Visibility::Inherited => {}
        _ => panic!("#[{typ}] methods must not have visibility modifiers"),
    }

    // No parameters allowed
    if !sig.inputs.is_empty() {
        panic!("#[{typ}] methods may not have parameters");
    }

    // No return type allowed
    match sig.output {
        syn::ReturnType::Default => {}
        _ => panic!("#[{typ}] methods must not have return types"),
    }
}
