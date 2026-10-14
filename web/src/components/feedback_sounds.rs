use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;
use web_sys::{AudioBuffer, AudioContext};

struct Tone {
    frequency: f64,
    duration: f64,
    delay: f64,
    kind: &'static str,
    envelope: f64,
}

fn tones(kind: &str) -> Vec<Tone> {
    match kind {
        "send" => vec![Tone { frequency: 720.0, duration: 0.055, delay: 0.0, kind: "sine", envelope: 1.0 }],
        "good" => vec![
            Tone { frequency: 740.0, duration: 0.08, delay: 0.0, kind: "sine", envelope: 1.0 },
            Tone { frequency: 988.0, duration: 0.1, delay: 0.07, kind: "sine", envelope: 1.0 },
        ],
        "almost" => vec![Tone { frequency: 520.0, duration: 0.11, delay: 0.0, kind: "triangle", envelope: 1.0 }],
        "bad" => vec![
            Tone { frequency: 360.0, duration: 0.085, delay: 0.0, kind: "bite", envelope: 2.4 },
            Tone { frequency: 250.0, duration: 0.095, delay: 0.075, kind: "bite", envelope: 2.4 },
        ],
        _ => vec![],
    }
}

fn gain_params(kind: &str) -> (f64, f64) {
    match kind {
        "send" => (0.75, 0.44),
        "good" => (0.9, 0.44),
        "almost" => (1.0, 0.44),
        "bad" => (0.38, 0.1),
        _ => (1.0, 0.44),
    }
}

fn render_tone(ctx: &AudioContext, tone: &Tone) -> Option<AudioBuffer> {
    let sample_rate = ctx.sample_rate() as f64;
    let frames = ((tone.delay + tone.duration) * sample_rate).ceil().max(1.0) as usize;
    let buffer = ctx.create_buffer(1, frames as u32, sample_rate as f32).ok()?;
    let mut channel = vec![0f32; frames];
    let delay_frames = (tone.delay * sample_rate) as usize;
    let tone_frames = (tone.duration * sample_rate).ceil().max(1.0) as usize;
    let mut peak = 0f32;
    for (i, slot) in channel
        .iter_mut()
        .skip(delay_frames)
        .take(tone_frames)
        .enumerate()
    {
        let t = i as f64 / sample_rate;
        let env = (1.0 - i as f64 / tone_frames as f64).max(0.0).powf(tone.envelope);
        let ph = 2.0 * PI * tone.frequency * t;
        let s = ph.sin();
        let sample = match tone.kind {
            "triangle" => (2.0 / PI) * s.asin(),
            "square" => {
                if s >= 0.0 {
                    1.0
                } else {
                    -1.0
                }
            }
            "bite" => (2.0 / PI) * s.asin() * 0.5 + if s >= 0.0 { 0.5 } else { -0.5 },
            _ => s,
        };
        let v = (sample * env) as f32;
        *slot = v;
        peak = peak.max(v.abs());
    }
    if peak > 0.0 {
        let scale = 0.98 / peak;
        for v in channel.iter_mut() {
            *v *= scale;
        }
    }
    buffer.copy_to_channel(&channel, 0).ok()?;
    Some(buffer)
}

thread_local! {
    static CTX: RefCell<Option<Rc<AudioContext>>> = const { RefCell::new(None) };
}

fn audio_context() -> Option<Rc<AudioContext>> {
    CTX.with(|c| {
        let mut slot = c.borrow_mut();
        if slot.is_none() {
            *slot = AudioContext::new().ok().map(Rc::new);
        }
        slot.clone()
    })
}

pub fn play_feedback(kind: &str, answer_sounds: bool, volume: f64) {
    if !answer_sounds || volume <= 0.0 {
        return;
    }
    let Some(ctx) = audio_context() else {
        return;
    };
    if ctx.state() == web_sys::AudioContextState::Suspended {
        let _ = ctx.resume();
    }
    let (mult, cap) = gain_params(kind);
    let level = ((0.18 + volume * 0.2) * mult).min(cap);
    if level <= 0.0 {
        return;
    }
    let Ok(gain) = ctx.create_gain() else { return };
    gain.gain().set_value(level as f32);
    let Ok(comp) = ctx.create_dynamics_compressor() else {
        return;
    };
    comp.threshold().set_value(-10.0);
    comp.knee().set_value(18.0);
    comp.ratio().set_value(4.0);
    comp.attack().set_value(0.002);
    comp.release().set_value(0.12);
    let _ = gain.connect_with_audio_node(&comp);
    let _ = comp.connect_with_audio_node(&ctx.destination());

    for tone in tones(kind) {
        let Some(buffer) = render_tone(&ctx, &tone) else {
            continue;
        };
        let Ok(src) = ctx.create_buffer_source() else {
            continue;
        };
        src.set_buffer(Some(&buffer));
        let _ = src.connect_with_audio_node(&gain);
        let _ = src.start();
    }
}
