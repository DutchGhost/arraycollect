use crate::FillError;

use core::mem::{self, MaybeUninit};
use core::ptr;

pub trait PartialArray {
    fn uninit() -> Self;
}

impl<T, const N: usize> PartialArray for [MaybeUninit<T>; N] {
    fn uninit() -> Self {
        unsafe { MaybeUninit::<_>::uninit().assume_init() }
    }
}

pub trait FromIter<T>: Sized {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Result<Self, FillError>;
}

struct ScopeExitGuard<T, Data, F>
where
    F: FnMut(&Data, &mut T),
{
    value: T,
    data: Data,
    f: F,
}

impl<T, Data, F> Drop for ScopeExitGuard<T, Data, F>
where
    F: FnMut(&Data, &mut T),
{
    fn drop(&mut self) {
        (self.f)(&self.data, &mut self.value);
    }
}

impl<T, const N: usize> FromIter<T> for [T; N] {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Result<Self, FillError> {
        // First create an uninitialized array of [MaybeUninit<T>; N].
        let mut partial = <[MaybeUninit<T>; N]>::uninit();

        // Then setup a scopeguard,
        // which should drop any already written items
        // if there is a panic during collecting,
        // or if the iterator has less elements than `N`.
        let mut guard = ScopeExitGuard {
            value: &mut partial[..],
            data: 0,

            f: move |&len, slice| {
                let slice: *mut [MaybeUninit<T>] = &mut slice[..len];
                let slice: *mut [T] = slice as *mut _;

                unsafe { ptr::drop_in_place(slice) }
            },
        };

        // Collect
        for (src, dst) in iter.into_iter().zip(guard.value.iter_mut()) {
            unsafe {
                ptr::write(dst.as_mut_ptr(), src);
                guard.data += 1;
            }
        }

        // if we wrote `N` items, we're good,
        // so make sure the guard doesnt drop,
        // and return the array.
        if guard.data == N {
            guard.value = &mut [];
            guard.data = 0;

            mem::forget(guard);

            // transmute_copy could be just a transmute,
            // but due to to the fact that the layout might differ,
            // we cant.
            // A case where the layout differs is Option<MaybeUninit<T>> -> Option<T>.
            // In our case, a [MaybeUninit<T>; N] doesnt differ from the layout of [T; N]/
            unsafe {
                let array: [T; N] = mem::transmute_copy(&partial);
                mem::forget(partial);
                Ok(array)
            }
        } else {
            // We're not good, so return an error.
            // The dropguard will run here.
            Err(FillError::new(guard.data, N))
        }
    }
}

pub trait IntoArray: Iterator {
    fn array_collect<A: FromIter<Self::Item>>(self) -> Result<A, FillError>
    where
        Self: Sized,
    {
        A::from_iter(self)
    }
}

impl<I: Iterator> IntoArray for I {}
