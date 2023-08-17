// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::buffer::{GpuBuffer, GpuBufferType};
use crate::renderer::Renderer;
use crate::vertex::Vertex;
use crate::{DefaultRender, PointBuffer, PointBufferBinding};
use std::borrow::{Borrow, Cow};
use std::collections::VecDeque;
use std::ops::Range;

/// Buffers that can back a [`Deque`].
#[doc(hidden)]
pub trait Buffer<P> {
    fn gpu_buffer_mut(&mut self) -> &mut GpuBuffer<P, { GpuBufferType::Array.to() }>;
}

/// Buffers that can be bound with no arguments (besides a [`Renderer`]).
#[doc(hidden)]
pub trait Bind {
    type Binding<'b>: BufferBinding + 'b
    where
        Self: 'b;
    fn bind<'a>(&'a self, renderer: &'a Renderer) -> Self::Binding<'a>;
}

/// Binding of a [`Buffer`] that is backing a [`Deque`].
#[doc(hidden)]
pub trait BufferBinding {
    fn draw(&self);
    fn draw_range(&self, range: Range<usize>);
}

impl<P: Vertex> Buffer<P> for PointBuffer<P> {
    fn gpu_buffer_mut(&mut self) -> &mut GpuBuffer<P, { GpuBufferType::Array.to() }> {
        &mut self.points
    }
}

impl<P: Vertex> Bind for PointBuffer<P> {
    type Binding<'b> = PointBufferBinding<'b, P>;

    fn bind<'a>(&'a self, renderer: &'a Renderer) -> Self::Binding<'a> {
        self.bind(renderer)
    }
}

impl<'a, P: Vertex> BufferBinding for PointBufferBinding<'a, P> {
    fn draw(&self) {
        self.draw();
    }

    fn draw_range(&self, range: Range<usize>) {
        self.draw_range(range);
    }
}

/// A [`Deque`] of points.
pub type PointDeque<P> = Deque<P, PointBuffer<P>>;

// Can't support a deque of instances in WebGL since draw_arrays_instanced_base_instance doesn't
// exist.
// pub type InstanceDeque<M> = Deque<M, InstanceBuffer<M>>;

/// It's analogous to [`VecDeque`] but on the GPU.
pub struct Deque<V, B> {
    backing_buffer: B,

    // Capacity, always a power of 2.
    capacity: usize,

    // Where data is read from backing_buffer.
    tail: usize,

    // Where data is written to backing_buffer.
    head: usize,

    // CPU buffer (required in WebGL because no copyBufferSubData).
    buffer: VecDeque<V>,

    // How many items were popped from the buffer since it was copied to the GPU.
    popped: usize,

    // How many items were pushed to the buffer since it was copied to the GPU.
    pushed: usize,
}

impl<V: Vertex, B: Buffer<V>> DefaultRender for Deque<V, B>
where
    B: DefaultRender,
{
    fn new(renderer: &Renderer) -> Self {
        Self {
            backing_buffer: DefaultRender::new(renderer),
            capacity: 0,
            tail: 0,
            head: 0,
            buffer: VecDeque::with_capacity(1024),
            popped: 0,
            pushed: 0,
        }
    }
}

impl<V: Vertex, B: Buffer<V>> Deque<V, B> {
    /// Pushes an element to the [`PointDeque`] (silently errors once there are 1,000,000 elements).
    pub fn push_back(&mut self, v: V) {
        if self.buffer.len() >= 1000000 {
            return;
        }
        self.pushed += 1;
        self.buffer.push_back(v);
    }

    /// Pops an element from the front of the [`Deque`], returning it.
    pub fn pop_front(&mut self) -> V {
        self.popped += 1;
        self.buffer.pop_front().unwrap()
    }

    /// Peeks the front of the [`PointDeque`].
    pub fn front(&self) -> Option<&V> {
        self.buffer.front()
    }

    /// Returns true if the [`PointDeque`] has no elements to draw.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Binds the [`PointDeque`] to draw its elements.
    #[must_use]
    pub fn bind<'a>(&'a mut self, renderer: &'a Renderer) -> DequeBinding<'a, V, B, B::Binding<'a>>
    where
        B: Bind,
    {
        self.bind_with(renderer, |b| b.bind(renderer))
    }

    /// Same as [`Bind`] but for buffers which require additional parameters to bind.
    #[must_use]
    pub fn bind_with<'a, X: BufferBinding>(
        &'a mut self,
        renderer: &'a Renderer,
        bind: impl FnOnce(&'a B) -> X,
    ) -> DequeBinding<'a, V, B, X> {
        self.buffer(renderer);

        DequeBinding {
            binding: bind(&self.backing_buffer),
            deque: self,
        }
    }

    /// Called by bind.
    fn buffer(&mut self, renderer: &Renderer) {
        if self.pushed == 0 && self.popped == 0 {
            return;
        }
        let gl = &renderer.gl;

        // Allocate gpu_buffer to the capacity of the VecDeque.
        let new_cap = self.buffer.capacity();
        let gpu_buffer = self.backing_buffer.gpu_buffer_mut();

        if new_cap > self.capacity {
            // Allocate new_cap on GPU.
            gpu_buffer.resize_zeroed(gl, new_cap);
            self.capacity = new_cap;

            // All data was deleted by grow so reset head and tail.
            self.tail = 0;
            self.head = self.buffer.len();

            let (a, b) = self.buffer.as_slices();

            let mut start = 0;
            for slice in [a, b] {
                if slice.is_empty() {
                    continue;
                }

                gpu_buffer.buffer_sub_data(gl, slice, start);
                start += slice.len();
            }
        } else if self.popped > 0 || self.pushed > 0 {
            debug_assert_eq!(self.capacity, self.capacity.next_power_of_two());

            let n = self.buffer.len();

            let new_pushed = self.pushed.min(n);
            if new_pushed != self.pushed {
                // However many we skip out of pushed, we skip out of popped.
                self.popped = self.popped.saturating_sub(self.pushed - new_pushed)
            }

            // Only push items that are still in the buffer.
            self.pushed = new_pushed;

            // Capacity is power of 2 so & works as a faster %.
            self.tail = (self.tail + self.popped) & (self.capacity - 1);

            let range = n - self.pushed..n;
            let (slice_a, slice_b) = self.buffer.as_slices();

            let vertices = if slice_b.len() >= self.pushed {
                // Slice b aka last items has all pushed items.
                Cow::Borrowed(&slice_b[slice_b.len() - self.pushed..])
            } else if slice_b.is_empty() {
                // Slice b is empty aka contiguous so slice a has all items.
                Cow::Borrowed(&slice_a[range])
            } else {
                // Items are split across 2 slices so allocation is needed.
                Cow::Owned(self.buffer.range(range).copied().collect())
            };

            let vertices: &[V] = vertices.borrow();
            if !vertices.is_empty() {
                // Space after head available.
                let available = self.capacity - self.head;
                let split = vertices.len().min(available);

                let (slice_a, slice_b) = vertices.split_at(split);
                let calls = [(slice_a, self.head), (slice_b, 0)];

                for (slice, start) in calls {
                    if slice.is_empty() {
                        continue;
                    }

                    gpu_buffer.buffer_sub_data(gl, slice, start)
                }
            }

            // Capacity is power of 2 so & works as a faster %.
            self.head = (self.head + self.pushed) & (self.capacity - 1);
        }

        self.pushed = 0;
        self.popped = 0;
    }
}

/// A bound [`PointDeque`] that can draw points.
pub struct DequeBinding<'a, V: Vertex, B: Buffer<V>, X> {
    binding: X,
    deque: &'a Deque<V, B>,
}

impl<'a, V: Vertex, B: Buffer<V>, X: BufferBinding> DequeBinding<'a, V, B, X> {
    /// Draws points.
    pub fn draw(&self) {
        if self.deque.tail <= self.deque.head {
            // Deque is contiguous.
            self.binding.draw_range(self.deque.tail..self.deque.head);
        } else {
            // [tail, len)
            self.binding
                .draw_range(self.deque.tail..self.deque.capacity);

            // [0, head)
            self.binding.draw_range(0..self.deque.head);
        }
    }
}
