#![no_std]

#[macro_export]
macro_rules! uninit_array {
    ($tgt:ty; $size:expr) => {
        unsafe { MaybeUninit::<[MaybeUninit<$tgt>; $size]>::uninit().assume_init() }
    };
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FillError;

#[macro_export]
macro_rules! arraycollect {
    ($iter:expr => [$tgt:ty; $size:tt]) => (
        {
            use ::core::mem::{self, MaybeUninit};
            use $crate::FillError;

            type Array = [$tgt; $size];
            
            struct PartialArray<T> {
                data: [MaybeUninit<T>; $size],
                filled: usize,
            }

            impl <T> PartialArray<T> {
                fn new() -> Self {
                    Self {
                        data: $crate::uninit_array!(T; $size),
                        filled: 0,
                    }
                }

                #[inline]
                fn is_filled(&self) -> bool {
                    self.filled == $size
                }

                fn collect<I>(mut self, iter: I) -> Result<[T; $size], $crate::FillError>
                where
                    I: Iterator<Item = T>
                {
                    for (src, dst) in iter.zip(self.data.iter_mut()) {
                        *dst = MaybeUninit::new(src);
                        self.filled += 1;
                    }

                    if self.is_filled() {
                        Ok(self.finish())
                    } else {
                        drop(self);
                        return Err(FillError)
                    }
                }

                fn finish(mut self) -> [T; $size] {
                    self.filled = 0;
                    unsafe {
                        let ptr = &mut self.data as *mut [MaybeUninit<T>; $size] as *mut [T; $size];
                        let rd = ::core::ptr::read(ptr);
                        mem::forget(self);
                        rd
                    }
                }
            }

            impl <T> Drop for PartialArray<T> {
                fn drop(&mut self) {
                    self.data[0..self.filled].iter_mut().map(|elem| {
                        let elem = mem::replace(elem, MaybeUninit::uninit());
                        unsafe {
                            elem.assume_init()
                        }
                    }).for_each(drop)
                }
            }
            
            PartialArray::<$tgt>::new()
                .collect($iter)
        }
    );
}
