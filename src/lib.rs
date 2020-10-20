#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![feature(
    const_generics,
    const_evaluatable_checked,
    const_fn,
)]
#![cfg_attr(
    feature = "const_init",
    feature(
        const_mut_refs,
        const_fn_fn_ptr_basics,
        const_panic,
        const_eval_limit,
    )
)]
#![cfg_attr(feature = "const_init", const_eval_limit = "10000000")]
#![allow(incomplete_features)]

pub mod tree;
