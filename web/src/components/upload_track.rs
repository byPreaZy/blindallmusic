//! `UploadTrack.vue` — local-track upload with a client-side 30s segment
//! picker: decode via `AudioContext`, waveform bars, draggable window,
//! peak-normalize to -1dB then MP3-encode through `window.lamejs`
//! (`/js/lame.all.js`, same CDN build the Blade template loads).
//! Multipart POST `/api/v1/local-tracks` (audio ~30s ±1s ≤1MB, artwork
//! image ≤2MB, artist_name + track_name).

use crate::{api, i18n::t};
use leptos::prelude::*;
use serde_json::Value;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

const SEGMENT: f64 = 30.0;

#[component]
pub fn UploadTrack(
    playlist_id: i64,
    on_uploaded: impl Fn(()) + 'static + Copy + Send + Sync,
) -> impl IntoView {
    let _ = playlist_id;
    let open = RwSignal::new(false);
    let artist = RwSignal::new(String::new());
    let track = RwSignal::new(String::new());
    let audio = RwSignal::new(Option::<web_sys::File>::None);
    let artwork = RwSignal::new(Option::<web_sys::File>::None);
    let artwork_url = RwSignal::new(Option::<String>::None);
    let audio_url = RwSignal::new(Option::<String>::None);
    let audio_el = NodeRef::<leptos::html::Audio>::new();
    let duration = RwSignal::new(0.0f64);
    let seg_start = RwSignal::new(0.0f64);
    let cur_time = RwSignal::new(0.0f64);
    let playing = RwSignal::new(false);
    let waveform = RwSignal::new(Vec::<f64>::new());
    let generating = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let processing = RwSignal::new(false);
    let err = RwSignal::new(Option::<String>::None);
    let wave_box = NodeRef::<leptos::html::Div>::new();
    // (dragging, grab_offset_seconds)
    let drag = RwSignal::new(Option::<f64>::None);

    let fmt = |s: f64| format!("{}:{:02}", (s / 60.0) as i64, (s % 60.0) as i64);

    let on_audio = move |ev: leptos::ev::Event| {
        let Some(f) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
            .and_then(|i| i.files())
            .and_then(|l| l.get(0))
        else {
            return;
        };
        err.set(None);
        audio.set(Some(f.clone()));
        if let Some(old) = audio_url.get_untracked() {
            web_sys::Url::revoke_object_url(&old).ok();
        }
        let url = web_sys::Url::create_object_url_with_blob(&f).unwrap_or_default();
        audio_url.set(Some(url.clone()));
        duration.set(0.0);
        seg_start.set(0.0);
        cur_time.set(0.0);
        waveform.set(vec![]);
        generating.set(true);
        leptos::task::spawn_local(async move {
            if let Ok((dur, bars)) = decode_waveform(&f).await {
                duration.set(dur);
                waveform.set(bars);
            }
            generating.set(false);
        });
    };

    let on_artwork = move |ev: leptos::ev::Event| {
        let Some(f) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
            .and_then(|i| i.files())
            .and_then(|l| l.get(0))
        else {
            return;
        };
        artwork.set(Some(f.clone()));
        if let Some(old) = artwork_url.get_untracked() {
            web_sys::Url::revoke_object_url(&old).ok();
        }
        artwork_url.set(web_sys::Url::create_object_url_with_blob(&f).ok());
    };

    let toggle_play = move |_| {
        let Some(el) = audio_el.get() else { return };
        if playing.get() {
            let _ = el.pause();
            playing.set(false);
        } else {
            el.set_current_time(seg_start.get());
            let _ = el.play();
            playing.set(true);
        }
    };

    // seconds -> pointer event clientX mapping
    let x_to_time = move |client_x: f64| -> f64 {
        let Some(el) = wave_box.get() else { return 0.0 };
        let rect = el.get_bounding_client_rect();
        if rect.width() <= 0.0 {
            return 0.0;
        }
        let pct = ((client_x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        pct * duration.get()
    };

    let clamp_seg = move |s: f64| s.clamp(0.0, (duration.get() - SEGMENT).max(0.0));

    view! {
        <button type="button"
            class="flex w-full items-center justify-center gap-2 rounded-lg border border-white/15 bg-white/5 px-4 py-2.5 text-sm font-medium text-white/80 transition hover:border-teal-500/50 hover:bg-white/10 hover:text-white"
            on:click=move |_| open.set(true)>
            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="size-5">
                <path stroke-linecap="round" stroke-linejoin="round" d="M9 8.25H7.5a2.25 2.25 0 0 0-2.25 2.25v9a2.25 2.25 0 0 0 2.25 2.25h9a2.25 2.25 0 0 0 2.25-2.25v-9a2.25 2.25 0 0 0-2.25-2.25H15m0-3-3-3m0 0-3 3m3-3V15"/>
            </svg>
            {t("Upload MP3 file")}
        </button>

        <Show when=move || open.get()>
            <div class="fixed inset-0 z-50 flex items-center justify-center px-4">
                <div class="retro-modal-overlay" on:click=move |_| open.set(false)></div>
                <div class="retro-modal-panel relative max-w-2xl w-full max-h-[90vh] overflow-y-auto p-6">
                    <h1 class="mb-2 text-2xl font-bold text-white">{t("Upload a song from your computer")}</h1>
                    <p class="mb-3 text-sm text-white/50">{t("Import an MP3 file from your computer. You can select a 30-second segment to use in the game")}</p>
                    <p class="mb-6 text-xs text-white/40">{t("All fields are required.")}</p>

                    <form class="space-y-5" on:submit=move |ev| {
                        ev.prevent_default();
                        let Some(audio_f) = audio.get() else {
                            err.set(Some(t("Please select an audio file").to_string()));
                            return;
                        };
                        let Some(art) = artwork.get() else {
                            err.set(Some(t("The artwork field is required").to_string()));
                            return;
                        };
                        let (a, tr, start) = (artist.get(), track.get(), seg_start.get());
                        busy.set(true);
                        processing.set(true);
                        err.set(None);
                        leptos::task::spawn_local(async move {
                            match extract_segment(&audio_f, start, SEGMENT).await {
                                Ok(mp3) => {
                                    processing.set(false);
                                    match post_track(&mp3, &art, &a, &tr).await {
                                        Ok(_) => {
                                            open.set(false);
                                            on_uploaded(());
                                        }
                                        Err(e) => err.set(Some(e)),
                                    }
                                }
                                Err(e) => err.set(Some(e)),
                            }
                            processing.set(false);
                            busy.set(false);
                        });
                    }>
                        <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
                            <div>
                                <label class="mb-1 block text-sm text-white/60">{t("Artist name")}</label>
                                <input type="text" required maxlength=255
                                    class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                                    prop:value=artist on:input=move |ev| artist.set(event_target_value(&ev))/>
                            </div>
                            <div>
                                <label class="mb-1 block text-sm text-white/60">{t("Track title")}</label>
                                <input type="text" required maxlength=255
                                    class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                                    prop:value=track on:input=move |ev| track.set(event_target_value(&ev))/>
                            </div>
                        </div>

                        <div class="rounded-lg border border-white/10 bg-white/5 p-4">
                            <label class="mb-1 block text-sm text-white/60">"Audio"</label>
                            <input type="file" accept="audio/mp3,audio/mpeg" required
                                class="w-full text-sm text-white/70 file:mr-3 file:rounded-lg file:border-0 file:bg-white/10 file:px-3 file:py-1.5 file:text-white"
                                on:change=on_audio/>
                            <p class="mt-2 text-xs text-red-400">{t("Audio files must be in MP3 format.")}</p>
                        </div>

                        <Show when=move || audio_url.get().is_some()>
                            <div class="space-y-3 rounded-lg border border-white/10 bg-white/5 p-4">
                                <h3 class="text-sm font-medium text-white">{t("Segment selection")}</h3>
                                <div class="flex items-center gap-4">
                                    <button type="button" on:click=toggle_play
                                        class="flex h-12 w-12 shrink-0 items-center justify-center rounded-full bg-white/10 text-white transition hover:bg-white/20">
                                        <Show when=move || playing.get() fallback=|| view! {
                                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-6 w-6">
                                                <path stroke-linecap="round" stroke-linejoin="round" d="M5.25 5.653c0-.856.917-1.398 1.667-.986l11.54 6.348a1.125 1.125 0 010 1.971l-11.54 6.347a1.125 1.125 0 01-1.667-.985V5.653z"/>
                                            </svg>
                                        }>
                                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-6 w-6">
                                                <path stroke-linecap="round" stroke-linejoin="round" d="M15.75 5.25v13.5m-7.5-13.5v13.5"/>
                                            </svg>
                                        </Show>
                                    </button>

                                    <div node_ref=wave_box
                                        class="relative h-20 w-full flex-1 cursor-pointer touch-none overflow-hidden rounded-xl bg-surface-base"
                                        on:pointerdown=move |ev| {
                                            ev.prevent_default();
                                            let t = x_to_time(ev.client_x() as f64);
                                            let s = seg_start.get();
                                            if t >= s && t <= s + SEGMENT {
                                                // grab the window — keep grab offset
                                                drag.set(Some(t - s));
                                                if let Some(el) = wave_box.get() {
                                                    let _ = el.set_pointer_capture(ev.pointer_id());
                                                }
                                            } else if let Some(el) = audio_el.get() {
                                                el.set_current_time(t);
                                                cur_time.set(t);
                                                if !playing.get() {
                                                    let _ = el.play();
                                                    playing.set(true);
                                                }
                                            }
                                        }
                                        on:pointermove=move |ev| {
                                            if let Some(off) = drag.get() {
                                                seg_start.set(clamp_seg(x_to_time(ev.client_x() as f64) - off));
                                            }
                                        }
                                        on:pointerup=move |_ev| {
                                            if drag.get().is_some() {
                                                drag.set(None);
                                                if let Some(el) = audio_el.get() {
                                                    el.set_current_time(seg_start.get());
                                                }
                                            }
                                        }>
                                        <Show when=move || generating.get()>
                                            <div class="absolute inset-0 z-10 flex items-center justify-center bg-surface-raised/70">
                                                <span class="text-xs text-white">{t("Generating waveform…")}</span>
                                            </div>
                                        </Show>
                                        <div class="pointer-events-none absolute inset-0 flex h-full w-full items-end gap-px">
                                            <For each=move || waveform.get() key=|v| (*v * 1000.0) as i64
                                                children=|v| view! {
                                                    <div class="flex-1 rounded-full bg-white/25" style=format!("height:{}%", (v * 100.0) as i32)></div>
                                                }
                                            />
                                        </div>
                                        // 30s window
                                        <div class="absolute inset-y-0 cursor-grab border-2 border-yellow-400 bg-yellow-300/20 active:cursor-grabbing"
                                            style=move || {
                                                let d = duration.get().max(1.0);
                                                format!("left:{}%;width:{}%", seg_start.get() / d * 100.0, SEGMENT.min(d) / d * 100.0)
                                            }>
                                        </div>
                                        // playhead
                                        <Show when=move || playing.get()>
                                            <div class="absolute bottom-0 top-0 w-0.5 bg-white"
                                                style=move || format!("left:{}%", cur_time.get() / duration.get().max(1.0) * 100.0)>
                                            </div>
                                        </Show>
                                        <div class="pointer-events-none absolute bottom-0 left-0 right-0 flex justify-between px-2 text-[10px] text-white/40">
                                            <span>"0:00"</span>
                                            <span>{move || fmt(duration.get())}</span>
                                        </div>
                                    </div>
                                </div>
                                <div class="flex justify-between rounded-lg bg-surface-base/60 p-2 text-xs text-white/50">
                                    <span>"Position: " {move || fmt(cur_time.get())}</span>
                                    <span>
                                        {t("Selection")}": " {move || fmt(seg_start.get())} " - " {move || fmt(seg_start.get() + SEGMENT)}
                                    </span>
                                </div>
                            </div>
                        </Show>

                        <div class="rounded-lg border border-white/10 bg-white/5 p-4">
                            <label class="mb-1 block text-sm text-white/60">{t("Album artwork")}</label>
                            <input type="file" accept="image/*" required
                                class="w-full text-sm text-white/70 file:mr-3 file:rounded-lg file:border-0 file:bg-white/10 file:px-3 file:py-1.5 file:text-white"
                                on:change=on_artwork/>
                            <p class="mt-2 text-xs text-red-400">{t("Image files must be PNG, JPG or JPEG and smaller than 2MB.")}</p>
                            <Show when=move || artwork_url.get().is_some()>
                                <div class="mt-4 flex items-center gap-3">
                                    <img src=move || artwork_url.get().unwrap_or_default() class="h-24 w-24 rounded-lg border border-white/10 object-cover" alt=""/>
                                    <p class="text-sm text-white/50">{t("This image will be used as the track artwork in the playlist.")}</p>
                                </div>
                            </Show>
                        </div>

                        {move || err.get().map(|e| view! {
                            <div class="rounded-lg border border-red-500/50 bg-red-500/20 p-3 text-sm text-red-400">{e}</div>
                        })}

                        <audio node_ref=audio_el src=move || audio_url.get().unwrap_or_default()
                            on:timeupdate=move |ev| {
                                let el = event_target::<web_sys::HtmlAudioElement>(&ev);
                                cur_time.set(el.current_time());
                                if el.current_time() >= seg_start.get() + SEGMENT {
                                    let _ = el.pause();
                                    playing.set(false);
                                }
                            }
                            on:ended=move |_| playing.set(false)
                            class="hidden"/>

                        <div class="flex justify-end">
                            <button type="submit" disabled=move || busy.get()
                                class="btn-primary disabled:cursor-not-allowed disabled:opacity-50">
                                {move || if processing.get() {
                                    t("Processing audio…").to_string()
                                } else if busy.get() {
                                    t("Uploading…").to_string()
                                } else {
                                    t("Import the clip into the Blind'All Music library").to_string()
                                }}
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </Show>
    }
}

/// Decode → (duration, 100-sample normalized waveform of channel 0).
async fn decode_waveform(f: &web_sys::File) -> Result<(f64, Vec<f64>), String> {
    let buf = audio_buffer(f).await?;
    let dur = buf.duration();
    let data = buf.get_channel_data(0).map_err(|e| format!("{e:?}"))?;
    let samples = 100usize;
    let block = (data.len() / samples).max(1);
    let mut bars = Vec::with_capacity(samples);
    for i in 0..samples {
        let start = i * block;
        let end = ((i + 1) * block).min(data.len());
        let sum: f32 = data[start..end].iter().map(|v| v.abs()).sum();
        bars.push((sum / (end - start) as f32) as f64);
    }
    let max = bars.iter().cloned().fold(0.0f64, f64::max).max(0.0001);
    Ok((dur, bars.iter().map(|v| v / max).collect()))
}

async fn audio_buffer(f: &web_sys::File) -> Result<web_sys::AudioBuffer, String> {
    let ctx = web_sys::AudioContext::new().map_err(|e| format!("{e:?}"))?;
    let ab = JsFuture::from(f.array_buffer())
        .await
        .map_err(|e| format!("{e:?}"))?;
    let decoded = JsFuture::from(
        ctx.decode_audio_data(&ab.dyn_into().map_err(|_| "arraybuffer")?)
            .map_err(|e| format!("{e:?}"))?,
    )
    .await
    .map_err(|e| format!("{e:?}"))?;
    decoded.dyn_into::<web_sys::AudioBuffer>().map_err(|_| "decode".into())
}

/// Extract `dur` seconds at `start`, peak-normalize to -1dB (0.89),
/// encode stereo MP3 @128k via `window.lamejs.Mp3Encoder`.
async fn extract_segment(f: &web_sys::File, start: f64, dur: f64) -> Result<web_sys::Blob, String> {
    let buf = audio_buffer(f).await?;
    let rate = buf.sample_rate() as usize;
    let chans = buf.number_of_channels() as usize;
    let start_s = (start * rate as f64) as usize;
    let dur_s = ((dur * rate as f64) as usize).min(buf.length() as usize - start_s);
    let mut left = vec![0f32; dur_s];
    let mut right = vec![0f32; dur_s];
    buf.copy_to_channel_with_start_in_channel(&mut left, 0, start_s as u32)
        .map_err(|e| format!("{e:?}"))?;
    if chans > 1 {
        buf.copy_to_channel_with_start_in_channel(&mut right, 1, start_s as u32)
            .map_err(|e| format!("{e:?}"))?;
    } else {
        right.copy_from_slice(&left);
    }
    let peak = left
        .iter()
        .chain(right.iter())
        .fold(0f32, |m, v| m.max(v.abs()));
    let gain = if peak > 0.0 { 0.89f32 / peak } else { 1.0 };
    left.iter_mut().for_each(|v| *v *= gain);
    right.iter_mut().for_each(|v| *v *= gain);

    let window = web_sys::window().ok_or("no window")?;
    let lamejs = js_sys::Reflect::get(&window, &"lamejs".into())
        .map_err(|_| "lamejs not loaded".to_string())?;
    let ctor = js_sys::Reflect::get(&lamejs, &"Mp3Encoder".into())
        .map_err(|_| "Mp3Encoder".to_string())?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| "Mp3Encoder ctor".to_string())?;
    let args = js_sys::Array::of3(&2.into(), &(rate as u32).into(), &128.into());
    let enc = js_sys::Reflect::construct(&ctor, &args).map_err(|e| format!("{e:?}"))?;
    let encode = js_sys::Reflect::get(&enc, &"encodeBuffer".into())
        .map_err(|e| format!("{e:?}"))?
        .dyn_into::<js_sys::Function>()
        .map_err(|e| format!("{e:?}"))?;
    let flush = js_sys::Reflect::get(&enc, &"flush".into())
        .map_err(|e| format!("{e:?}"))?
        .dyn_into::<js_sys::Function>()
        .map_err(|e| format!("{e:?}"))?;

    let block = 1152usize; // lamejs frame size
    let mut mp3: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < dur_s {
        let n = block.min(dur_s - i);
        let lc = js_sys::Int16Array::new_with_length(n as u32);
        let rc = js_sys::Int16Array::new_with_length(n as u32);
        let li: Vec<i16> = left[i..i + n].iter().map(|v| (v * 0x7fff as f32) as i16).collect();
        let ri: Vec<i16> = right[i..i + n].iter().map(|v| (v * 0x7fff as f32) as i16).collect();
        lc.copy_from(&li);
        rc.copy_from(&ri);
        let chunk = encode
            .call2(&enc, &lc, &rc)
            .map_err(|e| format!("{e:?}"))?;
        mp3.extend(int8_to_vec(&chunk));
        i += n;
    }
    let tail = flush.call0(&enc).map_err(|e| format!("{e:?}"))?;
    mp3.extend(int8_to_vec(&tail));

    let arr = js_sys::Uint8Array::new_with_length(mp3.len() as u32);
    arr.copy_from(&mp3);
    let bag = web_sys::BlobPropertyBag::new();
    bag.set_type("audio/mp3");
    web_sys::Blob::new_with_u8_array_sequence_and_options(&js_sys::Array::of1(&arr), &bag)
        .map_err(|e| format!("{e:?}"))
}

fn int8_to_vec(v: &wasm_bindgen::JsValue) -> Vec<u8> {
    if v.is_undefined() || v.is_null() {
        return vec![];
    }
    let len = js_sys::Reflect::get(v, &"length".into())
        .ok()
        .and_then(|l| l.as_f64())
        .unwrap_or(0.0) as u32;
    let view = js_sys::Uint8Array::new(v);
    let mut out = vec![0u8; len as usize];
    view.copy_to(&mut out);
    out
}

/// Multipart `POST /api/v1/local-tracks` — browser sets the boundary.
async fn post_track(
    mp3: &web_sys::Blob,
    artwork: &web_sys::File,
    artist: &str,
    track: &str,
) -> Result<(), String> {
    let form = web_sys::FormData::new().map_err(|e| format!("{e:?}"))?;
    form.append_with_blob_and_filename("audio", mp3, "segment.mp3")
        .map_err(|e| format!("{e:?}"))?;
    form.append_with_blob("artwork", artwork)
        .map_err(|e| format!("{e:?}"))?;
    form.append_with_str("artist_name", artist)
        .map_err(|e| format!("{e:?}"))?;
    form.append_with_str("track_name", track)
        .map_err(|e| format!("{e:?}"))?;

    let mut req = gloo_net::http::Request::post("/api/v1/local-tracks");
    if let Some(t) = api::stored_token() {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    let resp = req
        .body(form)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        let msg = body["errors"]
            .as_object()
            .and_then(|o| o.values().next())
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .or_else(|| body["message"].as_str())
            .unwrap_or("Upload failed")
            .to_string();
        Err(msg)
    }
}
