#![cfg_attr(not(feature = "std"), no_std)]

//! This crate provides a macro that collects an iterator into an array.
//! The way it works that the macro
//! creates a PartialArray struct declaration under the hood,
//! which internally makes use of the MaybeUninit type,
//! to safely construct an uninitialized array.
//! 
//! After the array is constructed, the array is filled
//! with elements yielded from the iterator.
//! 
//! Once the filling phase is complete,
//! a check is performed to determine whether
//! the array was completely filled or not.
//! If it's not completely filled, the already written elements
//! get dropped, and an error is returned.
//! If it's completely filled, the array is returned.

#[macro_export]
macro_rules! uninit_array {
    ($tgt:ty; $size:expr) => {
        unsafe { MaybeUninit::<[MaybeUninit<$tgt>; $size]>::uninit().assume_init() }
    };
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct FillError {
    filled: usize,
    capacity: usize,
}

impl FillError {
    pub fn new(filled: usize, capacity: usize) -> Self {
        Self { filled, capacity }
    }
}

impl core::fmt::Debug for FillError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "Failed to collect into an array of size {}. Wrote {} elements",
            self.capacity, self.filled
        ))
    }
}

impl core::fmt::Display for FillError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "Failed to collect into an array of size {}. Wrote {} elements",
            self.capacity, self.filled
        ))
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FillError {}

/// Tries to collect `$iter` into an array of type `[$tgt; $size]`.
/// If the iterator yields less than `$size` elements, and error is returned.
/// 
/// # Examples
/// ```
/// use arraycollect::arraycollect;
/// 
/// let array = arraycollect!(0..10 => [usize; 10]);
/// 
/// assert_eq!(array, Ok([0, 1, 2, 3, 4, 5, 6, 7, 8, 9]));
/// ```
/// 
/// We try to collect an iterator of 5 elements into an array of 20.
/// This results in an error.
/// ```
/// use arraycollect::{FillError, arraycollect};
/// 
/// let array = arraycollect!(0..5 => [usize; 20]);
/// // filled 5 elements of an array of 20.
/// assert_eq!(array, Err(FillError::new(5, 20)));
/// ```
#[macro_export]
macro_rules! arraycollect {
    ($iter:expr => [$tgt:ty; $size:tt]) => (
        {
            use ::core::mem::{self, MaybeUninit};

            type Array<T> = [T; $size];
            type UninitArray<T> = [MaybeUninit<T>; $size];

            struct PartialArray<T> {
                data: UninitArray<T>,
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

                /// Collects the Iterator into an array.
                /// Implementation wise, the steps look like this:
                /// Loop over the iterator, zipped with the (uninitialized) array.
                /// On each iteration, set the current element of the array
                /// to a new MaybeUninit::new(src), and increment the number
                /// of elements we've so far written.
                ///
                /// # Panic
                /// Whenever the iterator panics,
                /// the drop implementation will only drop
                /// the number of elements we've so far written,
                /// and won't drop uninitialized memory.
                ///
                /// # Error
                /// Also if the iterator is drained before the last element of the array
                /// is written, only the ammount of written elements is dropped,
                /// and an error is returned from this function.
                fn collect<I>(mut self, iter: I) -> Result<Self, $crate::FillError>
                where
                    I: Iterator<Item = T>
                {
                    for (src, dst) in iter.zip(self.data.iter_mut()) {
                        *dst = MaybeUninit::new(src);
                        self.filled += 1;
                    }

                    if self.is_filled() {
                        Ok(self)
                    } else {
                        let filled = self.filled;
                        Err($crate::FillError::new(filled, $size))
                    }
                }

                fn into_inner(mut self) -> [T; $size] {
                    /*
                        This function sets the length first to 0,
                        Whenever somehow drop is still called,
                        it won't do anything, because the length is 0.

                        After, it reads the array from self, moving out of self.
                        This has to be done with a ptr::read, because of the Drop impl.
                        When we've read the array, we mem::forget(self), so it's never dropped.
                    */
                    self.filled = 0;
                    unsafe {
                        let ptr = &mut self.data as *mut UninitArray<T> as *mut Array<T>;
                        let rd = ::core::ptr::read(ptr);
                        mem::forget(self);
                        rd
                    }
                }
            }

            impl <T> Drop for PartialArray<T> {
                fn drop(&mut self) {
                    /*
                        For every initialized element in the array:
                            - replace with MaybeUninit::uninit(),
                            - assume_init the element
                            - drop(element)
                    */
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
                .map(|partial_array| {
                    partial_array.into_inner()
                })
        }
    );
}
