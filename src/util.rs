pub fn boxed_array<const N: usize, T>(input: Vec<T>) -> Box<[T; N]> {
    assert_eq!(input.len(), N);
    let boxed_slice_ptr = Box::into_raw(input.into_boxed_slice());
    let array_ptr = boxed_slice_ptr as *mut [T; N];
    // SAFETY: Element type is unchanged, size of vector was same as N (any superfluous capacity
    // will be lost though)
    unsafe { Box::from_raw(array_ptr) }
}
