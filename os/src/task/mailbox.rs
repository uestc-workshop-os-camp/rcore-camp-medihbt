//! Mail Box of rCore OS. Implemented by medihbt.
//! A ring queue stoing message.

use alloc::vec::Vec;

const MAX_RINGBUF_SIZE: usize = 512;

/// Mail box: a ring buffer which throws message when full.
#[derive(Clone, Copy)]
pub struct MailBox {
    /// real ring buffer data
    pub ringbuf: RingBuffer,
}

impl MailBox {
    /// empty instance
    pub fn new()-> Self {
        Self { ringbuf: RingBuffer::empty() }
    }
}

/// Ring buffer.
#[derive(Clone, Copy)]
pub struct RingBuffer {
    data_: [u8; MAX_RINGBUF_SIZE],
    head_: usize,
    tail_: usize,
}

/// Status
/// Ring buffer status: full, empty, normal
#[derive(Copy, Clone, PartialEq)]
pub enum RingBufferStatus {
    /// Full ring buffer
    Full,
    /// Empty ring buffer
    Empty,
    /// Normal
    Normal,
}

impl RingBuffer {
    /// new an empty ring buffer
    pub fn empty()-> Self {
        Self {
            data_: [0; MAX_RINGBUF_SIZE],
            head_: 0,
            tail_: 0,
        }
    }

    /// Length of this ring buffer.
    pub fn length(&self)-> usize {
        if self.head_ < self.tail_ {
            self.head_ + MAX_RINGBUF_SIZE - self.tail_
        } else {
            self.head_ - self.tail_
        }
    }

    /// Check if this ring buffer is full.
    pub fn is_full(&self)-> bool {
        self.tail_ - self.head_ == 1 ||
        self.tail_ + MAX_RINGBUF_SIZE - self.head_ == 1
    }

    /// Check if this ring buffer is empty.
    pub fn is_empty(&self)-> bool {
        self.tail_ == self.head_
    }

    /// Status of this
    pub fn get_status(&self)-> RingBufferStatus {
        if self.is_full() {
            RingBufferStatus::Full
        } else if self.is_empty() {
            RingBufferStatus::Empty
        } else {
            RingBufferStatus::Normal
        }
    }

    /// append an u8 unit. Deprecate if this buffer is full.
    pub fn append_u8(&mut self, c: u8)-> RingBufferStatus {
        if self.is_full() {
            return RingBufferStatus::Full;
        }
        self.data_[self.head_] = c;
        self.head_ = if self.head_ == MAX_RINGBUF_SIZE - 1 { 0 } else { self.head_ + 1 };
        if self.is_full() {
            RingBufferStatus::Full
        } else {
            RingBufferStatus::Normal
        }
    }

    /// pop an u8 unit from buffer's front. Return nothing if buffer is empty.
    pub fn pop_front(&mut self)-> Option<u8> {
        if self.is_empty() {
            return None;
        }
        let ret = self.data_[self.tail_];
        self.tail_ = if self.tail_ == MAX_RINGBUF_SIZE - 1 { 0 } else { self.tail_ + 1 };
        Some(ret)
    }

    /// write bytes into mailbox. Returns bytes written in.
    pub fn push_bytes(&mut self, bytes: &[u8]) -> usize {
        let mut nbytes_pushed = 0;
        for &c in bytes {
            match self.append_u8(c) {
                RingBufferStatus::Full => break,
                _ => {}
            }
            nbytes_pushed += 1;
        }
        nbytes_pushed
    }

    /// read bytes from mailbox. Returns bytes read from.
    pub fn pop_bytes(&mut self, bytes: &mut [u8]) -> usize {
        let mut nbytes_poped = 0;
        for rc in bytes {
            match self.pop_front() {
                Some(ch) => *rc = ch,
                None => break
            }
            nbytes_poped += 1;
        }
        nbytes_poped
    }

    /// Write string to mailbox. Returns bytes .
    pub fn push_str(&mut self, str: &str)-> usize {
        self.push_bytes(str.as_bytes())
    }

    /// Read all content from a mailbox and return value
    pub fn pop_nbytes(&mut self, nbytes: usize) -> Vec<u8> {
        let mut ret = Vec::new();
        ret.resize(self.length().min(nbytes), 0);
        self.pop_bytes(ret.as_mut_slice());
        ret
    }
}