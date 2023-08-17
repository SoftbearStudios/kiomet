// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::visibility::VisibilityEvent;
use js_sys::ArrayBuffer;
use sprite_sheet::AudioSprite;
use std::cell::RefCell;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{future_to_promise, JsFuture};
use web_sys::{
    AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState, Event, GainNode,
    OscillatorNode, Response,
};

/// A macro-generated enum representing all audio sprites.
/// They each have an index associated with them to use as a key into a [`Vec`].
pub trait Audio: Copy + Debug + 'static {
    /// Returns the [`Audio`]'s unique identifier.
    fn index(self) -> usize;

    /// Returns path to the audio file containing all the audio.
    fn path() -> &'static str;

    /// Returns a static slice of [`AudioSprite`]s indexed by [`Audio::index`].
    fn sprites() -> &'static [AudioSprite];
}

/// Renders (plays) audio.
pub struct AudioPlayer<A: Audio> {
    inner: Rc<RefCell<Inner<A>>>,
}

struct Inner<A: Audio> {
    context: AudioContext,
    sfx_gain: GainNode,
    /// What SFX volume is or is ramping up/down to.
    sfx_volume_target: f32,
    music_gain: GainNode,
    /// What music volume is or is ramping up/down to.
    music_volume_target: f32,
    track: Option<AudioBuffer>,
    /// Audio indexed by [`Audio::index`].
    playing: Box<[Vec<AudioBufferSourceNode>]>,
    /// The game wants to mute all audio.
    muted_by_game: bool,
    /// Whether muted because the page is unfocused.
    muted_by_visibility: bool,
    /// Whether muted due to avoid conflicting with an advertisement.
    muted_by_ad: bool,
    /// Volume (kept up to date with the corresponding setting).
    volume_setting: f32,
    /// Music (kept up to date with the corresponding setting).
    music_setting: bool,
    spooky: PhantomData<A>,
}

impl<A: Audio> Default for AudioPlayer<A> {
    fn default() -> Self {
        let context = web_sys::AudioContext::new().expect("failed to create AudioConetxt");

        let sfx_gain = web_sys::GainNode::new(&context).expect("failed to create gain node");
        let music_gain = web_sys::GainNode::new(&context).expect("failed to create gain node");
        let _ = sfx_gain.connect_with_audio_node(&context.destination());
        let _ = music_gain.connect_with_audio_node(&context.destination());

        let mut inner = Inner {
            context,
            sfx_gain,
            music_gain,
            track: None,
            playing: vec![Vec::new(); std::mem::variant_count::<A>()].into_boxed_slice(),
            muted_by_game: false,
            muted_by_visibility: false,
            muted_by_ad: false,
            sfx_volume_target: 1.0,
            music_volume_target: 1.0,
            volume_setting: 0.0,
            music_setting: false,
            spooky: PhantomData,
        };

        inner.update_volume();

        let inner = Rc::new(RefCell::new(inner));

        let promise = js_hooks::window().fetch_with_str(A::path());
        let inner_clone = inner.clone();

        let _ = future_to_promise(async move {
            let response: Response = JsFuture::from(promise).await.unwrap().dyn_into().unwrap();
            let array_buffer: ArrayBuffer = JsFuture::from(response.array_buffer().unwrap())
                .await
                .unwrap()
                .dyn_into()
                .unwrap();

            // Note: Cannot yield while borrowing; otherwise will be borrowed elsewhere. Use a scope
            // to drop the first borrow.
            let promise = {
                let inner = inner_clone.borrow();
                JsFuture::from(inner.context.decode_audio_data(&array_buffer).unwrap())
            };

            match promise.await {
                Ok(res) => {
                    let track = res.dyn_into().unwrap();
                    inner_clone.borrow_mut().track = Some(track);
                }
                Err(_) => {
                    js_hooks::console_error!("failed to load audio track");
                }
            }

            Ok(JsValue::NULL)
        });

        Self { inner }
    }
}

/// References a playing sound based on an audio buffer.
#[must_use]
pub struct AudioBufferHandle {
    source: AudioBufferSourceNode,
    gain: GainNode,
}

const CENTS_PER_OCTAVE: f32 = 1200.0;

impl AudioBufferHandle {
    /// Get volume multiplier.
    pub fn volume(&self) -> f32 {
        self.gain.gain().value()
    }

    /// Set volume multiplier.
    pub fn set_volume(&self, volume: f32) {
        self.gain.gain().set_value(volume);
    }

    /// Get pitch shift in octaves.
    pub fn pitch(&self) -> f32 {
        self.source.detune().value() * (1.0 / CENTS_PER_OCTAVE)
    }

    /// Set pitch shift in octaves.
    pub fn set_pitch(&self, pitch: f32) {
        self.source.detune().set_value(pitch * CENTS_PER_OCTAVE);
    }
}

impl Drop for AudioBufferHandle {
    fn drop(&mut self) {
        let _ = self.source.stop();
    }
}

/// References a playing sound based on a tone.
#[must_use]
pub struct AudioToneHandle {
    source: OscillatorNode,
    gain: GainNode,
}

impl AudioToneHandle {
    /// Get volume multiplier.
    pub fn volume(&self) -> f32 {
        self.gain.gain().value()
    }

    /// Set volume multiplier.
    pub fn set_volume(&self, volume: f32) {
        self.gain.gain().set_value(volume);
    }

    /// Get pitch shift in octaves.
    pub fn pitch(&self) -> f32 {
        self.source.detune().value() * (1.0 / CENTS_PER_OCTAVE)
    }

    /// Set pitch shift in octaves.
    pub fn set_pitch(&self, pitch: f32) {
        self.source.detune().set_value(pitch * CENTS_PER_OCTAVE);
    }
}

impl Drop for AudioToneHandle {
    fn drop(&mut self) {
        let _ = self.source.stop();
    }
}

impl<A: Audio> AudioPlayer<A> {
    /*
    pub fn sample_rate(&self) -> usize {
        self.inner.borrow().context.sample_rate() as usize
    }

    pub fn visit_sfx_gain(&self, mut visitor: impl FnMut(&GainNode)) {
        let inner = self.inner.borrow();
        visitor(&inner.sfx_gain);
    }
    */

    /// Creates an audio handle for a particular tone (with frequency in Hz).
    pub fn create_tone_sfx(&self, frequency: f32) -> AudioToneHandle {
        let inner = self.inner.borrow();
        let source = inner.context.create_oscillator().unwrap();
        source.frequency().set_value(frequency);
        let gain = inner.context.create_gain().unwrap();
        let _ = source.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(&inner.sfx_gain);
        let _ = source.start();
        AudioToneHandle { source, gain }
    }

    pub fn ramp_tone_volume(&self, tone: &AudioToneHandle, volume: f32) {
        let inner = self.inner.borrow();
        Inner::<A>::ramp(&tone.gain, volume, inner.context.current_time(), 0.05);
    }

    /// Creates an audio handle for brown noise.
    pub fn create_brown_noise_sfx(&self) -> AudioBufferHandle {
        let inner = self.inner.borrow();
        let sample_rate = 44100; // inner.context.sample_rate();
        let seconds = 1;

        let mut last = 0.0;
        let buf = Vec::from_iter(
            std::iter::repeat_with(|| {
                let white = js_sys::Math::random() as f32 * 2.0 - 1.0;
                let next = (last + (0.02 * white)) * (1.0 / 1.02);
                last = next;
                next * 3.5 // gain compensation.
            })
            .take(seconds * sample_rate),
        );
        let buffer = inner
            .context
            .create_buffer(1, buf.len() as u32, sample_rate as f32)
            .unwrap();
        let _ = buffer.copy_to_channel(&buf, 0);
        let source = inner.context.create_buffer_source().unwrap();
        source.set_buffer(Some(&buffer));
        source.set_loop(true);

        let gain = inner.context.create_gain().unwrap();
        let _ = source.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(&inner.sfx_gain);

        let _ = source.start();

        AudioBufferHandle { source, gain }
    }

    /// Plays a particular sound once.
    pub fn play(&self, audio: A) {
        self.play_with_volume(audio, 1.0);
    }

    /// Plays a particular sound once, with a specified volume.
    pub fn play_with_volume(&self, audio: A, volume: f32) {
        Inner::play(&self.inner, audio, volume);
    }

    /// Plays a particular sound once, with a specified volume and delay in seconds.
    pub fn play_with_volume_and_delay(&self, audio: A, volume: f32, _delay: f32) {
        Inner::play(&self.inner, audio, volume);
    }

    pub fn is_playing(&self, audio: A) -> bool {
        self.inner.borrow().is_playing(audio)
    }

    pub fn stop_playing(&self, audio: A) {
        self.inner.borrow_mut().stop_playing(audio);
    }

    // Sets a multiplier for the volume of all sounds.
    pub(crate) fn set_volume_setting(&self, volume_setting: f32, music_setting: bool) {
        let mut inner = self.inner.borrow_mut();
        inner.volume_setting = volume_setting;
        inner.music_setting = music_setting;
        inner.update_volume();
    }

    /// For the game to mute/unmute all audio.
    pub fn set_muted_by_game(&self, muted_by_game: bool) {
        let mut inner = self.inner.borrow_mut();
        inner.muted_by_game = muted_by_game;
        inner.update_volume();
    }

    pub(crate) fn peek_visibility(&self, event: &VisibilityEvent) {
        let mut inner = self.inner.borrow_mut();
        inner.muted_by_visibility = match event {
            VisibilityEvent::Visible(visible) => !visible,
        };
        inner.update_volume();
    }

    pub fn set_muted_by_ad(&self, muted_by_ad: bool) {
        let mut inner = self.inner.borrow_mut();
        inner.muted_by_ad = muted_by_ad;
        inner.update_volume();
    }
}

impl<A: Audio> Inner<A> {
    fn recalculate_volume(&self, music: bool) -> f32 {
        if self.muted_by_game
            || self.muted_by_visibility
            || self.muted_by_ad
            || (music && !self.music_setting)
        {
            0.0
        } else {
            self.volume_setting
        }
    }

    fn ramp(gain: &GainNode, volume: f32, current_time: f64, delay: f64) {
        if delay <= 0.0
            || gain
                .gain()
                .linear_ramp_to_value_at_time(volume, current_time + delay)
                .is_err()
        {
            let _ = gain.gain().cancel_scheduled_values(current_time);
            gain.gain().set_value(volume);
        }
    }

    fn update_volume(&mut self) {
        for music in [false, true] {
            let new_volume = self.recalculate_volume(music);
            let (gain, volume_target) = if music {
                (&self.music_gain, &mut self.music_volume_target)
            } else {
                (&self.sfx_gain, &mut self.sfx_volume_target)
            };
            if new_volume != *volume_target {
                *volume_target = new_volume;

                let time = self.context.current_time();
                let delay = if new_volume <= 0.0 { 0.0 } else { 1.5 };
                Self::ramp(gain, new_volume, time, delay);
            }
        }

        if self.context.state() == AudioContextState::Suspended {
            let _ = self.context.resume();
        }
    }

    /// Plays a particular sound, optionally in a loop. This is private, since looping is never
    /// determined at runtime.
    fn play(rc: &Rc<RefCell<Self>>, audio: A, volume: f32) {
        let mut inner = rc.borrow_mut();
        if inner.recalculate_volume(false) == 0.0 {
            return;
        }

        if inner.context.state() == AudioContextState::Suspended {
            let _ = inner.context.resume();
        } else if let Some(track) = inner.track.as_ref() {
            let sprite = &A::sprites()[audio.index()];
            if inner.recalculate_volume(sprite.music) == 0.0 {
                return;
            }

            let source: AudioBufferSourceNode = inner
                .context
                .create_buffer_source()
                .unwrap()
                .dyn_into()
                .unwrap();

            source.set_buffer(Some(track));

            let gain = web_sys::GainNode::new(&inner.context).unwrap();
            gain.gain().set_value(volume);
            let _ = source.connect_with_audio_node(&gain);

            let _ = gain.connect_with_audio_node(if sprite.music {
                &inner.music_gain
            } else {
                &inner.sfx_gain
            });

            if sprite.looping {
                source.set_loop(true);
                source.set_loop_start(sprite.loop_start.unwrap_or(sprite.start) as f64);
                source.set_loop_end((sprite.start + sprite.duration) as f64);
                let _ = source.start_with_when_and_grain_offset(0.0, sprite.start as f64);
            } else {
                let _ = source.start_with_when_and_grain_offset_and_grain_duration(
                    0.0,
                    sprite.start as f64,
                    sprite.duration as f64,
                );
            }

            let cloned_rc = Rc::clone(rc);
            let stop = Closure::once_into_js(move |value: JsValue| {
                let event: Event = value.dyn_into().unwrap();
                let mut inner = cloned_rc.borrow_mut();
                let playing = &mut inner.playing[audio.index()];
                for source in playing.drain_filter(|p| {
                    *p == event
                        .target()
                        .unwrap()
                        .dyn_into::<AudioBufferSourceNode>()
                        .unwrap()
                }) {
                    // Ensure no double-invocation.
                    source.set_onended(None);
                }
            });

            source.set_onended(Some(stop.as_ref().unchecked_ref()));

            inner.playing[audio.index()].push(source);
        }
    }

    fn is_playing(&self, audio: A) -> bool {
        !self.playing[audio.index()].is_empty()
    }

    fn stop_playing(&mut self, audio: A) {
        let playing = &mut self.playing[audio.index()];
        for removed in playing.drain(..) {
            // WebAudio bug makes unsetting loop required?
            removed.set_loop(false);
            let _ = removed.stop();
        }
    }
}
