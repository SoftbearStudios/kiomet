// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::apply::Apply;
use common_util::ticks::TicksTrait;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

pub struct UnJitter<S, U, T> {
    inner: S,
    queue: VecDeque<U>,
    time_since_last_tick: f32,
    _spooky: PhantomData<T>,
}

impl<S: Default, U, T> Default for UnJitter<S, U, T> {
    fn default() -> Self {
        Self {
            inner: Default::default(),
            queue: Default::default(),
            time_since_last_tick: Default::default(),
            _spooky: PhantomData,
        }
    }
}

impl<S, U, T> Deref for UnJitter<S, U, T> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<S, U, T> DerefMut for UnJitter<S, U, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<S: Apply<U>, U, T: TicksTrait> UnJitter<S, U, T> {
    pub fn update(&mut self, elapsed_time: f32) {
        self.time_since_last_tick += elapsed_time;
        self.maybe_apply();
    }

    pub fn time_since_last_tick(&self) -> f32 {
        self.time_since_last_tick
    }

    fn maybe_apply(&mut self) {
        if !self.queue.is_empty() {
            let delay = self.delay();
            if self.time_since_last_tick >= delay {
                self.inner.apply(self.queue.pop_back().unwrap());
                self.time_since_last_tick =
                    (self.time_since_last_tick - delay).clamp(0.0, 2.0 * T::PERIOD_SECS);
            }
        }
    }

    fn delay(&self) -> f32 {
        (match self.queue.len() {
            0 => unreachable!(),
            1 => 1.025,
            2 => 0.95,
            3 => 0.75,
            _ => 0.0,
        }) * T::PERIOD_SECS
    }
}

impl<S: Apply<U>, U, T: TicksTrait> Apply<U> for UnJitter<S, U, T> {
    fn apply(&mut self, update: U) {
        self.queue.push_front(update);
        self.maybe_apply();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::sync_channel;
    use std::thread;

    use crate::apply::Apply;
    use crate::un_jitter::UnJitter;
    use common_util::ticks::GenTicks;
    use rand::{thread_rng, Rng};
    use std::ops::{Deref, DerefMut};
    use std::sync::{atomic::AtomicBool, Arc, Mutex};
    use std::time::{Duration, Instant};
    type Ticks = GenTicks<10>;

    #[derive(Default)]
    struct State {
        value: f32,
        rate: f32,
        total_latency: Duration,
        max_latency: Duration,
    }

    impl State {
        fn value(&self, time_since_last_tick: f32) -> f32 {
            self.value + time_since_last_tick * self.rate
        }
    }

    #[derive(Default)]
    struct Naive {
        state: State,
        time_since_last_tick: f32,
    }

    impl Apply<Update> for Naive {
        fn apply(&mut self, update: Update) {
            self.state.apply(update);
            self.time_since_last_tick = 0.0;
        }
    }

    impl Naive {
        fn update(&mut self, elapsed_seconds: f32) {
            self.time_since_last_tick += elapsed_seconds;
        }

        fn value(&self) -> f32 {
            self.state.value(self.time_since_last_tick)
        }
    }

    impl Deref for Naive {
        type Target = State;

        fn deref(&self) -> &State {
            &self.state
        }
    }

    impl DerefMut for Naive {
        fn deref_mut(&mut self) -> &mut State {
            &mut self.state
        }
    }

    #[derive(Copy, Clone)]
    struct Update {
        rate: f32,
        timestamp: Instant,
    }

    impl Apply<Update> for State {
        fn apply(&mut self, update: Update) {
            self.value = self.value(Ticks::PERIOD_SECS);
            self.rate = update.rate;
            let elapsed = update.timestamp.elapsed();
            self.total_latency += elapsed;
            self.max_latency = self.max_latency.max(elapsed);
        }
    }

    #[test]
    fn interpolation() {
        let server: Arc<Mutex<State>> = Default::default();
        let raw_client: Arc<Mutex<Naive>> = Default::default();
        let un_jitter_client: Arc<Mutex<UnJitter<State, Update, Ticks>>> = Default::default();
        let (sender, receiver) = sync_channel(1000);
        let done = Arc::new(AtomicBool::default());

        let server_thread = {
            let server = server.clone();
            thread::spawn(move || {
                let mut rng = thread_rng();

                for _ in 0..25 {
                    thread::sleep(Duration::from_secs_f32(Ticks::PERIOD_SECS));

                    let mut server = server.lock().unwrap();

                    let update = Update {
                        rate: server.rate + 0.3 * rng.gen_range(-1f32..1f32),
                        timestamp: Instant::now(),
                    };

                    server.apply(update);

                    sender.send(update).unwrap();
                }
            })
        };

        let network_thread = {
            let raw_client = raw_client.clone();
            let un_jitter_client = un_jitter_client.clone();
            let done = done.clone();

            thread::spawn(move || {
                let mut budget = 0.0; // Ticks::PERIOD_SECS;

                while let Ok(update) = receiver.recv() {
                    if true {
                        budget += Ticks::PERIOD_SECS;
                        let mut rng = thread_rng();
                        let delay = rng.gen_range(0.0..budget);
                        budget -= delay;
                        thread::sleep(Duration::from_secs_f32(delay));
                    } else {
                        thread::sleep(Duration::from_secs_f32(Ticks::PERIOD_SECS));
                    }

                    raw_client.lock().unwrap().apply(update);
                    un_jitter_client.lock().unwrap().apply(update);
                }

                done.store(true, Ordering::Relaxed);
            })
        };

        let clients_thread = {
            let frame = 0.016;

            thread::spawn(move || {
                let mut i = 0;
                let mut last = Instant::now();
                while !done.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_secs_f32(frame));

                    let server = server.lock().unwrap();
                    let mut raw_client = raw_client.lock().unwrap();
                    let mut un_jitter_client = un_jitter_client.lock().unwrap();

                    let elapsed = last.elapsed().as_secs_f32();
                    raw_client.update(elapsed);
                    un_jitter_client.update(elapsed);
                    last = Instant::now();

                    println!(
                        "{i}, {}, {}, {}",
                        server.value,
                        raw_client.value(),
                        un_jitter_client.value(un_jitter_client.time_since_last_tick()),
                        //un_jitter_client.queue.len(),
                        //un_jitter_client.time_since_last_tick() / Ticks::PERIOD_SECS,
                    );

                    i += 1;
                }

                let raw_client = raw_client.lock().unwrap();
                let un_jitter_client = un_jitter_client.lock().unwrap();

                println!(
                    "total latency raw: {:?}, un_jitter: {:?}",
                    raw_client.total_latency, un_jitter_client.total_latency
                );
                println!(
                    "max latency raw: {:?}, un_jitter: {:?}",
                    raw_client.max_latency, un_jitter_client.max_latency
                );
            })
        };

        server_thread.join().unwrap();
        network_thread.join().unwrap();
        clients_thread.join().unwrap();
    }
}
