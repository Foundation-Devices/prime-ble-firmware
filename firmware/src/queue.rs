use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub struct BytesQueue<const N: usize, const M: usize> {
    head: AtomicUsize,
    tail: AtomicUsize,
    slots: [Slot<M>; N],
}

struct Slot<const M: usize> {
    buf: UnsafeCell<[u8; M]>,
    len: AtomicUsize,
    full: AtomicBool,
}

impl<const M: usize> Slot<M> {
    const fn new() -> Self {
        Self {
            buf: UnsafeCell::new([0; M]),
            len: AtomicUsize::new(0),
            full: AtomicBool::new(false),
        }
    }
}

impl<const N: usize, const M: usize> BytesQueue<N, M> {
    pub const fn new() -> Self {
        Self {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            slots: [const { Slot::new() }; N],
        }
    }

    #[inline]
    fn next(idx: usize) -> usize {
        (idx + 1) % N
    }

    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }

    pub fn is_full(&self) -> bool {
        Self::next(self.tail.load(Ordering::Acquire)) == self.head.load(Ordering::Acquire)
    }

    /// Producer borrows a free slot.
    pub fn send(&self) -> Option<&mut [u8; M]> {
        if self.is_full() {
            return None;
        }
        let tail = self.tail.load(Ordering::Relaxed);
        let slot = &self.slots[tail];
        if slot.full.load(Ordering::Acquire) {
            return None;
        }
        // SAFETY: only producer touches this slot until send_done
        Some(unsafe { &mut *slot.buf.get() })
    }

    pub fn send_done(&self, len: usize) {
        let tail = self.tail.load(Ordering::Relaxed);
        let slot = &self.slots[tail];
        slot.len.store(len, Ordering::Release);
        slot.full.store(true, Ordering::Release);
        self.tail.store(Self::next(tail), Ordering::Release);
    }

    /// Consumer borrows the next filled slot.
    pub fn receive(&self) -> Option<&[u8]> {
        if self.is_empty() {
            return None;
        }
        let head = self.head.load(Ordering::Relaxed);
        let slot = &self.slots[head];
        if !slot.full.load(Ordering::Acquire) {
            return None;
        }
        let len = slot.len.load(Ordering::Acquire);
        let buf = unsafe { &*slot.buf.get() };
        Some(&buf[..len])
    }

    pub fn receive_done(&self) {
        let head = self.head.load(Ordering::Relaxed);
        let slot = &self.slots[head];
        slot.full.store(false, Ordering::Release);
        self.head.store(Self::next(head), Ordering::Release);
    }
}

// Safety: SPSC only — one producer thread, one consumer thread.
unsafe impl<const N: usize, const M: usize> Sync for BytesQueue<N, M> {}
