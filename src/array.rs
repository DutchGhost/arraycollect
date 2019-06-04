use crate::FillError;

use core::mem::{self, MaybeUninit};
use core::ptr;

pub trait Array {
    /// The array's element type.
    type Item;

    /// The uninit array.
    type PartialArray: PartialArray<Element = <Self as Array>::Item>;

    fn as_mut_slice(&mut self) -> &mut [Self::Item];
}

pub trait PartialArray: Array<Item = MaybeUninit<<Self as PartialArray>::Element>> {
    type Element;

    fn uninit() -> Self;
}

impl<T, const N: usize> Array for [T; N] {
    type Item = T;

    type PartialArray = [MaybeUninit<T>; N];

    fn as_mut_slice(&mut self) -> &mut [Self::Item] {
        self
    }
}

impl<T, const N: usize> PartialArray for [MaybeUninit<T>; N] {
    type Element = T;

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
        let mut partial = <[T; N] as Array>::PartialArray::uninit();

        // Then setup a scopeguard,
        // which should drop any already written items
        // if there is a panic during collecting,
        // or if the iterator has less elements than `N`.
        let mut guard = ScopeExitGuard {
            value: partial.as_mut_slice(),
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
                ptr::write(dst, MaybeUninit::new(src));
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

            let array: [T; N] = unsafe {
                let ptr: *const [MaybeUninit<T>; N] = &partial;
                let ptr: *const [T; N] = ptr as _;
                ptr::read(ptr)
            };

            Ok(array)
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
