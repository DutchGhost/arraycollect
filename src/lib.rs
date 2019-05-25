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

            /// This is a PartialArray, which allows some elements
            /// to be initialized, and some elements to not be initialized.
            ///
            /// This allows collecting an iterator into an array,
            /// because we can safely make an uninitialized array,
            /// and then fill it with elements of the iterator as we go.
            #[repr(C)]
            struct PartialArray<T> {
                data: UninitArray<T>,
                filled: usize,
            }

            /// This is a FilledArray, which is used to
            /// move the fully initialized array out of a PartialArray.
            ///
            /// We have to do it this way, because PartialArray
            /// implements Drop, so we cant move out of it.
            ///
            /// We transmute a PartialArray to this struct,
            /// and then move out of this struct instead.
            #[repr(C)]
            struct FilledArray<T> {
                data: Array<T>,
                _filled: usize,
            }

            impl <T> FilledArray<T> {
                #[inline(always)]
                fn data(self) -> Array<T> {
                    self.data
                }
            }

            impl <T> PartialArray<T> {
                fn new() -> Self {
                    Self {
                        data: $crate::uninit_array!(T; $size),
                        filled: 0,
                    }
                }

                #[inline]
                const fn is_filled(&self) -> bool {
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
            }

            impl <T> Drop for PartialArray<T> {
                fn drop(&mut self) {
                    //for elem in &mut self.data[..self.filled] {
                    //    unsafe {
                    //        ::core::ptr::drop_in_place(elem.as_mut_ptr());
                    //    }
                    //}

                    let len = self.filled;

                    let ptr: *mut MaybeUninit<T> = self.data[..len].as_mut_ptr();
                    let ptr = ptr as *mut T;

                    unsafe {
                        let slice = ::core::slice::from_raw_parts_mut(ptr, len);
                        ::core::ptr::drop_in_place::<[T]>(slice);
                    }
                }
            }

            impl PartialArray<$tgt> {
                /// This function sets the length to 0,
                /// to avoid drop doing any work if it's somehow invoked.
                /// Then it simply transmute's self, hence its marked unsafe.
                ///
                /// Transmuting a PartialArray<$tgt> to a FilledArray<$tgt>
                /// is actually safe, because they both have the same layout,
                /// enforced by #[repr(C)].
                #[inline(always)]
                unsafe fn into_filled(mut self) -> FilledArray<$tgt> {
                    self.filled = 0;
                    mem::transmute(self)
                }

                /// Transforms the partial array into a fully initialized array.
                fn into_array(mut self) -> Array<$tgt> {
                    unsafe { self.into_filled().data() }
                }
            }

            PartialArray::<$tgt>::new()
                .collect($iter)
                .map(|partial_array| {
                    partial_array.into_array()
                })
        }
    );
}
