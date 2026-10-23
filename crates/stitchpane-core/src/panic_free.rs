//! Panic-free forms of standard library and `image` operations that panic on bad input
//! and are banned in `clippy.toml` (RUST-001).

use image::{ImageBuffer, Pixel, Primitive};

/// `clamp` without its panic when `min > max` or a bound is NaN: the result is
/// `self.max(min).min(max)`, so such a range gives `max`.
pub trait Clamped: PartialOrd + Sized {
    fn clamped(self, min: Self, max: Self) -> Self {
        let low = if self < min { min } else { self };
        if low > max { max } else { low }
    }
}

impl<T: PartialOrd> Clamped for T {}

/// Removes and returns the element at `index`, or `None` when it is out of range.
pub(crate) fn remove_at<T>(items: &mut Vec<T>, index: usize) -> Option<T> {
    if index >= items.len() {
        return None;
    }
    // move the element to the end, keeping the order of the others, then pop it
    let mut rest = items.get_mut(index..)?.iter_mut();
    if let Some(mut prev) = rest.next() {
        for next in rest {
            std::mem::swap(prev, next);
            prev = next;
        }
    }
    items.pop()
}

/// Inserts `item` at `index`, shifting later elements up; gives `item` back when
/// `index` is past the end.
pub(crate) fn insert_at<T>(items: &mut Vec<T>, index: usize, item: T) -> Result<(), T> {
    if index > items.len() {
        return Err(item);
    }
    items.push(item);
    // move the new last element down to `index`
    if let Some(tail) = items.get_mut(index..) {
        let mut rest = tail.iter_mut().rev();
        if let Some(mut prev) = rest.next() {
            for next in rest {
                std::mem::swap(prev, next);
                prev = next;
            }
        }
    }
    Ok(())
}

/// Splits `slice` into pieces of `size` (at least 1); the last may be shorter.
pub(crate) fn chunks<T>(mut rest: &[T], size: usize) -> impl Iterator<Item = &[T]> {
    let size = size.max(1);
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let (chunk, tail) = rest.split_at_checked(size.min(rest.len()))?;
        rest = tail;
        Some(chunk)
    })
}

/// Mutable form of [`chunks`].
pub(crate) fn chunks_mut<T>(mut rest: &mut [T], size: usize) -> impl Iterator<Item = &mut [T]> {
    let size = size.max(1);
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let taken = std::mem::take(&mut rest);
        let len = taken.len();
        let (chunk, tail) = taken.split_at_mut_checked(size.min(len))?;
        rest = tail;
        Some(chunk)
    })
}

/// Copies `src` into the start of `dst`; `false` (copying nothing) when `dst` is shorter.
pub(crate) fn copy_into<T: Copy>(dst: &mut [T], src: &[T]) -> bool {
    let Some(dst) = dst.get_mut(..src.len()) else {
        return false;
    };
    for (d, s) in dst.iter_mut().zip(src) {
        *d = *s;
    }
    true
}

/// Builds an image from a function of each pixel's position, like
/// `ImageBuffer::from_fn`; `None` when the image is too large to address.
pub(crate) fn image_from_fn<P: Pixel>(
    width: u32,
    height: u32,
    mut f: impl FnMut(u32, u32) -> P,
) -> Option<ImageBuffer<P, Vec<P::Subpixel>>> {
    let len = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(usize::from(P::CHANNEL_COUNT))?;
    let mut data = Vec::with_capacity(len);
    for y in 0..height {
        for x in 0..width {
            data.extend_from_slice(f(x, y).channels());
        }
    }
    ImageBuffer::from_raw(width, height, data)
}

/// A transparent (all-zero) image, like `ImageBuffer::new`; `None` when the image is
/// too large to address.
pub(crate) fn blank_image<P: Pixel>(
    width: u32,
    height: u32,
) -> Option<ImageBuffer<P, Vec<P::Subpixel>>> {
    let len = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(usize::from(P::CHANNEL_COUNT))?;
    ImageBuffer::from_raw(
        width,
        height,
        vec![<P::Subpixel as Primitive>::DEFAULT_MIN_VALUE; len],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_and_insert_keep_order() {
        let mut v = vec![1, 2, 3, 4];
        assert_eq!(remove_at(&mut v, 1), Some(2));
        assert_eq!(v, [1, 3, 4]);
        assert_eq!(remove_at(&mut v, 3), None);
        assert_eq!(insert_at(&mut v, 0, 9), Ok(()));
        assert_eq!(v, [9, 1, 3, 4]);
        assert_eq!(insert_at(&mut v, 4, 5), Ok(()));
        assert_eq!(v, [9, 1, 3, 4, 5]);
        assert_eq!(insert_at(&mut v, 9, 7), Err(7));
    }

    #[test]
    fn chunks_match_std() {
        let v: Vec<u8> = (0..10).collect();
        let ours: Vec<&[u8]> = chunks(&v, 3).collect();
        let std: Vec<&[u8]> = v.chunks(3).collect();
        assert_eq!(ours, std);
        assert_eq!(chunks(&v, 0).count(), 10);
        let mut w = v.clone();
        assert_eq!(
            chunks_mut(&mut w, 4).map(|c| c.len()).collect::<Vec<_>>(),
            [4, 4, 2]
        );
    }
}
