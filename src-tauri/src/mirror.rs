//! Native phone-screen answerer.
//!
//! WKWebView answers this phone's baseline H.264 (`420033`) as constrained
//! baseline and never decodes a frame. This peer answers `420033` itself and
//! hands JPEG frames to the window.

use crate::engine::Engine;
use base64::Engine as _;
use openh264::decoder::Decoder;
use openh264::formats::YUVSource;
use openh264::nal_units;
use serde::Serialize;
use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_H264};
use webrtc::api::APIBuilder;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::dtls_transport::dtls_role::DTLSRole;
use webrtc::ice_transport::ice_candidate::{RTCIceCandidate, RTCIceCandidateInit};
use webrtc::ice_transport::ice_candidate_type::RTCIceCandidateType;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::ice_transport::ice_protocol::RTCIceProtocol;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication;
use webrtc::rtp::codecs::h264::H264Packet;
use webrtc::rtp::packetizer::Depacketizer;
use webrtc::rtp_transceiver::rtp_codec::{RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType};
use webrtc::rtp_transceiver::RTCPFeedback;
use webrtc_ice::mdns::MulticastDnsMode;
use webrtc_ice::network_type::NetworkType;

const H264_FMTP: &str = "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=420033";
/// Longest side of the picture painted in the window. The phone frame is much taller than this.
const DISPLAY_EDGE: u32 = 960;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorView {
    pub seq: u64,
    pub jpeg: String,
    pub width: u32,
    pub height: u32,
    pub state: String,
    pub session_id: String,
}

struct LocalIce {
    candidate: String,
    label: i64,
    mid: String,
}

struct RemoteIce {
    candidate: String,
    label: i64,
    mid: String,
}

struct Slot {
    generation: u64,
    session_id: String,
    pc: Option<Arc<RTCPeerConnection>>,
    remote_ready: bool,
    answered: bool,
    gather_done: bool,
    local: Vec<LocalIce>,
    early: Vec<RemoteIce>,
    frame_seq: u64,
    jpeg: String,
    width: u32,
    height: u32,
    state: String,
    noted_up: bool,
    noted_video: bool,
    noted_fail: bool,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            generation: 0,
            session_id: String::new(),
            pc: None,
            remote_ready: false,
            answered: false,
            gather_done: false,
            local: Vec::new(),
            early: Vec::new(),
            frame_seq: 0,
            jpeg: String::new(),
            width: 0,
            height: 0,
            state: "idle".to_string(),
            noted_up: false,
            noted_video: false,
            noted_fail: false,
        }
    }
}

pub struct Mirror {
    inner: Mutex<Slot>,
}

impl Mirror {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { inner: Mutex::new(Slot::default()) })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Slot> {
        self.inner.lock().unwrap_or_else(|err| err.into_inner())
    }

    /// Starts a visible session. The previous peer, if any, is returned so the caller can close it.
    pub fn begin(&self, session_id: &str) -> Option<Arc<RTCPeerConnection>> {
        let mut slot = self.lock();
        slot.generation = slot.generation.wrapping_add(1);
        slot.session_id = session_id.to_string();
        slot.remote_ready = false;
        slot.answered = false;
        slot.gather_done = false;
        slot.local.clear();
        slot.early.clear();
        slot.jpeg.clear();
        slot.width = 0;
        slot.height = 0;
        slot.frame_seq = slot.frame_seq.wrapping_add(1);
        slot.state = "answering".to_string();
        slot.noted_up = false;
        slot.noted_video = false;
        slot.noted_fail = false;
        slot.pc.take()
    }

    pub fn owns(&self, session_id: &str) -> bool {
        let slot = self.lock();
        !slot.session_id.is_empty() && slot.session_id == session_id
    }

    pub fn view(&self, since: u64) -> MirrorView {
        let slot = self.lock();
        let jpeg = if slot.frame_seq == since { String::new() } else { slot.jpeg.clone() };
        MirrorView {
            seq: slot.frame_seq,
            jpeg,
            width: slot.width,
            height: slot.height,
            state: slot.state.clone(),
            session_id: slot.session_id.clone(),
        }
    }

    pub async fn close(&self, session_id: &str) {
        let pc = {
            let mut slot = self.lock();
            if slot.session_id != session_id {
                return;
            }
            slot.session_id.clear();
            slot.remote_ready = false;
            slot.answered = false;
            slot.local.clear();
            slot.early.clear();
            slot.jpeg.clear();
            slot.width = 0;
            slot.height = 0;
            slot.frame_seq = slot.frame_seq.wrapping_add(1);
            slot.state = "closed".to_string();
            slot.generation = slot.generation.wrapping_add(1);
            slot.pc.take()
        };
        if let Some(pc) = pc {
            let _ = pc.close().await;
        }
    }

    pub async fn add_remote(&self, session_id: &str, candidate: &str, label: i64, mid: &str) {
        let candidate = candidate.trim().trim_start_matches("a=").trim();
        if candidate.is_empty() || candidate.contains(".local") {
            return;
        }
        if !candidate.to_ascii_lowercase().contains(" udp ") {
            return;
        }
        let pc = {
            let mut slot = self.lock();
            if slot.session_id != session_id {
                return;
            }
            if !slot.remote_ready {
                slot.early.push(RemoteIce {
                    candidate: candidate.to_string(),
                    label,
                    mid: mid.to_string(),
                });
                return;
            }
            slot.pc.clone()
        };
        if let Some(pc) = pc {
            push_remote(&pc, candidate, label, mid).await;
        }
    }

    pub async fn answer(self: Arc<Self>, engine: Engine, session_id: String, sdp: String, lan: String, phone: String) {
        let generation = self.lock().generation;
        if !self.same(generation, &session_id) {
            return;
        }
        let Some(lan_ip) = lan.parse::<IpAddr>().ok().filter(|ip| !ip.is_unspecified()) else {
            self.fail(generation, &engine, "This Mac has no address on the phone's network.");
            return;
        };
        let offer_sdp = without_rtx(&sdp);
        if !offer_sdp.to_ascii_lowercase().contains("h264/90000") {
            self.fail(generation, &engine, "The phone offered no H.264 picture.");
            return;
        }
        let mid = video_mid(&offer_sdp);
        let pc = match new_peer(lan_ip).await {
            Ok(pc) => pc,
            Err(err) => {
                self.fail(generation, &engine, &format!("The phone screen did not start. {err}"));
                return;
            }
        };
        self.watch_link(generation, &engine, &pc);
        self.watch_local(generation, &engine, &pc, lan, mid.clone());
        self.watch_video(generation, &engine, &pc);
        let stale = {
            let mut slot = self.lock();
            if slot.generation != generation {
                true
            } else {
                slot.pc = Some(pc.clone());
                false
            }
        };
        if stale {
            let _ = pc.close().await;
            return;
        }
        let offer = match RTCSessionDescription::offer(offer_sdp) {
            Ok(offer) => offer,
            Err(err) => {
                self.fail(generation, &engine, &format!("The phone screen did not start. {}", public_error(&err)));
                self.drop_pc(generation).await;
                return;
            }
        };
        if let Err(err) = pc.set_remote_description(offer).await {
            self.fail(generation, &engine, &format!("The phone screen did not start. {}", public_error(&err)));
            self.drop_pc(generation).await;
            return;
        }
        let early = {
            let mut slot = self.lock();
            if slot.generation != generation {
                return;
            }
            slot.remote_ready = true;
            std::mem::take(&mut slot.early)
        };
        for item in early {
            push_remote(&pc, &item.candidate, item.label, &item.mid).await;
        }
        let answer = match pc.create_answer(None).await {
            Ok(answer) => answer,
            Err(err) => {
                self.fail(generation, &engine, &format!("The phone screen did not start. {}", public_error(&err)));
                self.drop_pc(generation).await;
                return;
            }
        };
        if let Err(err) = pc.set_local_description(answer).await {
            self.fail(generation, &engine, &format!("The phone screen did not start. {}", public_error(&err)));
            self.drop_pc(generation).await;
            return;
        }
        for _ in 0..16 {
            if !self.same(generation, &session_id) {
                return;
            }
            let ready = {
                let slot = self.lock();
                !slot.local.is_empty() || slot.gather_done
            };
            if ready {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let Some(local) = pc.local_description().await else {
            self.fail(generation, &engine, "The phone screen did not start. The screen answer was empty.");
            self.drop_pc(generation).await;
            return;
        };
        if !self.same(generation, &session_id) {
            return;
        }
        if video_rejected(&local.sdp) {
            engine.screen_note("This Mac rejected the phone's video format.");
        } else if !local.sdp.to_ascii_lowercase().contains("profile-level-id=420033") {
            engine.screen_note("The screen answer did not name the phone's H.264 profile.");
        }
        if let Err(err) = engine.webrtc_answer(&session_id, &local.sdp) {
            if self.same(generation, &session_id) {
                self.fail(generation, &engine, &format!("The phone screen did not start. {}", public_error(&err)));
            }
            return;
        }
        let pending = {
            let mut slot = self.lock();
            if slot.generation != generation {
                return;
            }
            slot.answered = true;
            if slot.state != "picture" && slot.state != "video" && slot.state != "connected" {
                slot.state = "connecting".to_string();
            }
            std::mem::take(&mut slot.local)
        };
        let mut sent = 0u32;
        for item in pending {
            engine.webrtc_ice(&session_id, &item.candidate, item.label, &item.mid);
            sent += 1;
        }
        if self.same(generation, &session_id) {
            if sent == 0 {
                engine.screen_note("This Mac found no screen address on the phone's network.");
            } else if sent == 1 {
                engine.screen_note("Sent 1 screen address to the phone.");
            } else {
                engine.screen_note(&format!("Sent {sent} screen addresses to the phone."));
            }
        }
        let _ = phone;
    }

    fn same(&self, generation: u64, session_id: &str) -> bool {
        let slot = self.lock();
        slot.generation == generation && slot.session_id == session_id
    }

    fn fail(&self, generation: u64, engine: &Engine, text: &str) {
        {
            let mut slot = self.lock();
            if slot.generation != generation {
                return;
            }
            slot.state = "failed".to_string();
        }
        engine.screen_note(text);
    }

    async fn drop_pc(&self, generation: u64) {
        let pc = {
            let mut slot = self.lock();
            if slot.generation != generation {
                return;
            }
            slot.pc.take()
        };
        if let Some(pc) = pc {
            let _ = pc.close().await;
        }
    }

    fn watch_link(self: &Arc<Self>, generation: u64, engine: &Engine, pc: &Arc<RTCPeerConnection>) {
        let engine_ice = engine.clone();
        let mirror = Arc::clone(self);
        pc.on_ice_connection_state_change(Box::new(move |state: RTCIceConnectionState| {
            let engine = engine_ice.clone();
            let mirror = mirror.clone();
            Box::pin(async move {
                match state {
                    RTCIceConnectionState::Connected | RTCIceConnectionState::Completed => {
                        let note = {
                            let mut slot = mirror.lock();
                            if slot.generation != generation || slot.noted_up {
                                false
                            } else {
                                slot.noted_up = true;
                                if slot.state != "picture" && slot.state != "video" {
                                    slot.state = "connected".to_string();
                                }
                                true
                            }
                        };
                        if note {
                            engine.screen_note("The screen link is up.");
                        }
                    }
                    RTCIceConnectionState::Failed => {
                        let note = {
                            let mut slot = mirror.lock();
                            if slot.generation != generation || slot.noted_fail {
                                false
                            } else {
                                slot.noted_fail = true;
                                slot.state = "failed".to_string();
                                true
                            }
                        };
                        if note {
                            engine.screen_note("The screen link failed.");
                        }
                    }
                    _ => {}
                }
            })
        }));
        let engine_peer = engine.clone();
        let mirror = Arc::clone(self);
        pc.on_peer_connection_state_change(Box::new(move |state: RTCPeerConnectionState| {
            let engine = engine_peer.clone();
            let mirror = mirror.clone();
            Box::pin(async move {
                if state != RTCPeerConnectionState::Failed {
                    return;
                }
                let note = {
                    let mut slot = mirror.lock();
                    if slot.generation != generation || slot.noted_fail {
                        false
                    } else {
                        slot.noted_fail = true;
                        slot.state = "failed".to_string();
                        true
                    }
                };
                if note {
                    engine.screen_note("The screen link failed.");
                }
            })
        }));
    }

    fn watch_local(self: &Arc<Self>, generation: u64, engine: &Engine, pc: &Arc<RTCPeerConnection>, lan: String, mid: String) {
        let engine = engine.clone();
        let mirror = Arc::clone(self);
        pc.on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| {
            let engine = engine.clone();
            let mirror = mirror.clone();
            let lan = lan.clone();
            let mid = mid.clone();
            Box::pin(async move {
                let Some(candidate) = candidate else {
                    let mut slot = mirror.lock();
                    if slot.generation == generation {
                        slot.gather_done = true;
                    }
                    return;
                };
                if candidate.typ != RTCIceCandidateType::Host || candidate.protocol != RTCIceProtocol::Udp || candidate.address != lan {
                    return;
                }
                let Ok(mut init) = candidate.to_json() else { return };
                if !host_on_lan(&init.candidate, &lan) {
                    return;
                }
                init.sdp_mid = Some(mid.clone());
                init.sdp_mline_index = Some(0);
                let item = LocalIce { candidate: init.candidate, label: 0, mid };
                let session_id = {
                    let mut slot = mirror.lock();
                    if slot.generation != generation {
                        return;
                    }
                    if !slot.answered {
                        slot.local.push(item);
                        return;
                    }
                    slot.session_id.clone()
                };
                engine.webrtc_ice(&session_id, &item.candidate, item.label, &item.mid);
            })
        }));
    }

    fn watch_video(self: &Arc<Self>, generation: u64, engine: &Engine, pc: &Arc<RTCPeerConnection>) {
        let engine = engine.clone();
        let mirror = Arc::clone(self);
        let weak = Arc::downgrade(pc);
        pc.on_track(Box::new(move |track, _, _| {
            let engine = engine.clone();
            let mirror = mirror.clone();
            let weak = weak.clone();
            Box::pin(async move {
                let mime = track.codec().capability.mime_type.to_ascii_lowercase();
                if !mime.contains("h264") {
                    return;
                }
                let media_ssrc = track.ssrc();
                tokio::spawn(async move {
                    let mut result = Ok(0usize);
                    while result.is_ok() {
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        let Some(pc) = weak.upgrade() else { break };
                        result = pc
                            .write_rtcp(&[Box::new(PictureLossIndication { sender_ssrc: 0, media_ssrc })])
                            .await
                            .map(|_| 0usize);
                    }
                });
                let pending = Arc::new(Mutex::new(None));
                let notify = Arc::new(tokio::sync::Notify::new());
                let finished = Arc::new(AtomicBool::new(false));
                spawn_publisher(generation, &engine, &mirror, Arc::clone(&pending), Arc::clone(&notify), Arc::clone(&finished));
                let mut packetizer = H264Packet::default();
                let mut decoder = match Decoder::new() {
                    Ok(decoder) => decoder,
                    Err(_) => {
                        engine.screen_note("The picture decoder did not start.");
                        finished.store(true, Ordering::Relaxed);
                        notify.notify_one();
                        return;
                    }
                };
                let mut decode_errors = 0u32;
                let mut saw_picture = false;
                loop {
                    let read = track.read_rtp().await;
                    let Ok((rtp, _)) = read else { break };
                    if mirror.mark_video(generation) {
                        engine.screen_note("Video is arriving.");
                    }
                    let Ok(payload) = packetizer.depacketize(&rtp.payload) else { continue };
                    if payload.is_empty() {
                        continue;
                    }
                    for nal in nal_list(&payload) {
                        match decoder.decode(nal) {
                            Ok(Some(yuv)) => {
                                let (width, height) = yuv.dimensions();
                                if width == 0 || height == 0 || width > 4000 || height > 4000 {
                                    continue;
                                }
                                // One frame waiting is enough. A queue here shows up as growing delay.
                                let occupied = pending.lock().unwrap_or_else(|err| err.into_inner()).is_some();
                                if occupied {
                                    saw_picture = true;
                                    decode_errors = 0;
                                    continue;
                                }
                                let source_w = width as u32;
                                let source_h = height as u32;
                                let (shown_w, shown_h) = display_size(source_w, source_h);
                                let Some(rgb) = sample_display(&yuv, shown_w, shown_h) else { continue };
                                *pending.lock().unwrap_or_else(|err| err.into_inner()) = Some(PhoneFrame {
                                    rgb,
                                    shown_w,
                                    shown_h,
                                    width: source_w,
                                    height: source_h,
                                });
                                notify.notify_one();
                                saw_picture = true;
                                decode_errors = 0;
                            }
                            Ok(None) => {}
                            Err(_) => decode_errors = decode_errors.saturating_add(1),
                        }
                    }
                    if decode_errors == 40 && !saw_picture {
                        engine.screen_note("Video is arriving, but the picture could not be decoded.");
                    }
                }
                finished.store(true, Ordering::Relaxed);
                notify.notify_one();
            })
        }));
    }

    fn live(&self, generation: u64) -> bool {
        self.lock().generation == generation
    }

    fn mark_video(&self, generation: u64) -> bool {
        let mut slot = self.lock();
        if slot.generation != generation || slot.noted_video {
            return false;
        }
        slot.noted_video = true;
        if slot.state != "picture" {
            slot.state = "video".to_string();
        }
        true
    }

    fn store_jpeg(&self, generation: u64, jpeg: String, width: u32, height: u32) -> Option<(u64, bool, String)> {
        let mut slot = self.lock();
        if slot.generation != generation {
            return None;
        }
        let first = slot.state != "picture";
        slot.frame_seq = slot.frame_seq.wrapping_add(1);
        let seq = slot.frame_seq;
        let session_id = slot.session_id.clone();
        slot.jpeg = jpeg;
        slot.width = width;
        slot.height = height;
        slot.state = "picture".to_string();
        Some((seq, first, session_id))
    }
}

struct PhoneFrame {
    rgb: Vec<u8>,
    shown_w: u32,
    shown_h: u32,
    width: u32,
    height: u32,
}

fn spawn_publisher(
    generation: u64,
    engine: &Engine,
    mirror: &Arc<Mirror>,
    pending: Arc<Mutex<Option<PhoneFrame>>>,
    notify: Arc<tokio::sync::Notify>,
    finished: Arc<AtomicBool>,
) {
    let engine = engine.clone();
    let mirror = Arc::clone(mirror);
    tokio::spawn(async move {
        let mut noted_blank = false;
        loop {
            if !mirror.live(generation) {
                break;
            }
            let frame = pending.lock().unwrap_or_else(|err| err.into_inner()).take();
            let Some(frame) = frame else {
                if finished.load(Ordering::Relaxed) {
                    break;
                }
                notify.notified().await;
                continue;
            };
            if !mirror.live(generation) {
                break;
            }
            let PhoneFrame { rgb, shown_w, shown_h, width, height } = frame;
            // A blank virtual display decodes as a black frame and covers the window.
            let blank = rgb.chunks(3).all(|pixel| pixel.iter().all(|channel| *channel < 8));
            if blank {
                if !noted_blank {
                    noted_blank = true;
                    engine.screen_note("The picture is blank.");
                }
                continue;
            }
            let encoded = tokio::task::spawn_blocking(move || encode_jpeg(&rgb, shown_w, shown_h).ok()).await.ok().flatten();
            let Some(jpeg) = encoded else { continue };
            let b64 = base64::engine::general_purpose::STANDARD.encode(jpeg);
            let Some((seq, first, session_id)) = mirror.store_jpeg(generation, b64.clone(), width, height) else { continue };
            engine.show_frame(seq, &b64, width, height, &session_id);
            if first {
                engine.screen_note("The phone's picture arrived.");
            }
        }
    });
}

fn display_size(width: u32, height: u32) -> (u32, u32) {
    let long = width.max(height);
    if width == 0 || height == 0 || long <= DISPLAY_EDGE {
        return (width, height);
    }
    let dw = (u64::from(width) * u64::from(DISPLAY_EDGE) + u64::from(long) / 2) / u64::from(long);
    let dh = (u64::from(height) * u64::from(DISPLAY_EDGE) + u64::from(long) / 2) / u64::from(long);
    (dw.max(1) as u32, dh.max(1) as u32)
}

fn sample_display(yuv: &impl YUVSource, dw: u32, dh: u32) -> Option<Vec<u8>> {
    let (sw, sh) = yuv.dimensions();
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return None;
    }
    let (ys, us, vs) = yuv.strides();
    if ys == 0 || us == 0 || vs == 0 {
        return None;
    }
    let y_plane = yuv.y();
    let u_plane = yuv.u();
    let v_plane = yuv.v();
    let mut out = vec![0u8; dw as usize * dh as usize * 3];
    for row in 0..dh as usize {
        let sy = row * sh / dh as usize;
        let uv_row = sy / 2;
        for col in 0..dw as usize {
            let sx = col * sw / dw as usize;
            let yi = sy * ys + sx;
            let ui = uv_row * us + sx / 2;
            let vi = uv_row * vs + sx / 2;
            if yi >= y_plane.len() || ui >= u_plane.len() || vi >= v_plane.len() {
                continue;
            }
            let yy = i32::from(y_plane[yi]);
            let du = i32::from(u_plane[ui]) - 128;
            let dv = i32::from(v_plane[vi]) - 128;
            let o = (row * dw as usize + col) * 3;
            out[o] = (yy + ((1436 * dv) >> 10)).clamp(0, 255) as u8;
            out[o + 1] = (yy - ((352 * du + 731 * dv) >> 10)).clamp(0, 255) as u8;
            out[o + 2] = (yy + ((1815 * du) >> 10)).clamp(0, 255) as u8;
        }
    }
    Some(out)
}

fn encode_jpeg(rgb: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ()> {
    if rgb.len() != width as usize * height as usize * 3 {
        return Err(());
    }
    let mut out = Vec::with_capacity(rgb.len() / 6);
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 55);
    encoder.encode(rgb, width, height, image::ExtendedColorType::Rgb8).map_err(|_| ())?;
    Ok(out)
}

fn nal_list(buf: &[u8]) -> Vec<&[u8]> {
    let found: Vec<&[u8]> = nal_units(buf).collect();
    if found.is_empty() && !buf.is_empty() {
        vec![buf]
    } else {
        found
    }
}

async fn push_remote(pc: &RTCPeerConnection, candidate: &str, label: i64, mid: &str) {
    let label = u16::try_from(label).unwrap_or(0);
    let attempts = [
        RTCIceCandidateInit {
            candidate: candidate.to_string(),
            sdp_mid: if mid.is_empty() { None } else { Some(mid.to_string()) },
            sdp_mline_index: Some(label),
            username_fragment: None,
        },
        RTCIceCandidateInit {
            candidate: candidate.to_string(),
            sdp_mid: None,
            sdp_mline_index: Some(label),
            username_fragment: None,
        },
    ];
    for init in attempts {
        if pc.add_ice_candidate(init).await.is_ok() {
            return;
        }
    }
}

async fn new_peer(lan: IpAddr) -> Result<Arc<RTCPeerConnection>, String> {
    let mut media = MediaEngine::default();
    media
        .register_codec(
            RTCRtpCodecParameters {
                capability: RTCRtpCodecCapability {
                    mime_type: MIME_TYPE_H264.to_owned(),
                    clock_rate: 90000,
                    channels: 0,
                    sdp_fmtp_line: H264_FMTP.to_owned(),
                    rtcp_feedback: vec![
                        RTCPFeedback { typ: "nack".to_owned(), parameter: String::new() },
                        RTCPFeedback { typ: "nack".to_owned(), parameter: "pli".to_owned() },
                    ],
                },
                payload_type: 102,
                ..Default::default()
            },
            RTPCodecType::Video,
        )
        .map_err(|err| public_error(&err))?;
    let mut registry = Registry::new();
    registry = register_default_interceptors(registry, &mut media).map_err(|err| public_error(&err))?;
    let mut setting = SettingEngine::default();
    setting.set_network_types(vec![NetworkType::Udp4]);
    setting.set_ice_multicast_dns_mode(MulticastDnsMode::Disabled);
    setting.set_ip_filter(Box::new(move |ip: IpAddr| ip == lan));
    let _ = setting.set_answering_dtls_role(DTLSRole::Client);
    let api = APIBuilder::new()
        .with_setting_engine(setting)
        .with_media_engine(media)
        .with_interceptor_registry(registry)
        .build();
    api.new_peer_connection(RTCConfiguration { ice_servers: vec![], ..Default::default() })
        .await
        .map(Arc::new)
        .map_err(|err| public_error(&err))
}

fn video_mid(sdp: &str) -> String {
    let mut video = false;
    for line in sdp.split(['\r', '\n']) {
        if let Some(rest) = line.strip_prefix("m=") {
            video = rest.starts_with("video ");
        } else if video {
            if let Some(mid) = line.strip_prefix("a=mid:") {
                return mid.trim().to_string();
            }
        }
    }
    "0".to_string()
}

fn video_rejected(sdp: &str) -> bool {
    sdp.split(['\r', '\n']).any(|line| line.starts_with("m=video 0"))
}

pub(crate) fn host_on_lan(candidate: &str, lan: &str) -> bool {
    if lan.is_empty() || candidate.contains(".local") {
        return false;
    }
    let text = candidate.trim().trim_start_matches("a=");
    let mut parts = text.split_whitespace();
    let _foundation = parts.next();
    let _component = parts.next();
    let protocol = parts.next().unwrap_or("");
    let _priority = parts.next();
    let address = parts.next().unwrap_or("");
    let _port = parts.next();
    let typ_key = parts.next().unwrap_or("");
    let typ = parts.next().unwrap_or("");
    protocol.eq_ignore_ascii_case("udp") && typ_key == "typ" && typ.eq_ignore_ascii_case("host") && address == lan
}

fn without_rtx(sdp: &str) -> String {
    let normalized = sdp.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<String> = normalized.split('\n').filter(|line| !line.is_empty()).map(str::to_string).collect();
    let mut rtx = HashSet::new();
    for line in &lines {
        if let Some(rest) = line.strip_prefix("a=rtpmap:") {
            let mut bits = rest.split_whitespace();
            let pt = bits.next().unwrap_or("");
            let codec = bits.next().unwrap_or("");
            if codec.to_ascii_lowercase().starts_with("rtx/") {
                rtx.insert(pt.to_string());
            }
        }
        if let Some(rest) = line.strip_prefix("a=fmtp:") {
            if rest.to_ascii_lowercase().contains("apt=") {
                if let Some(pt) = rest.split_whitespace().next() {
                    rtx.insert(pt.to_string());
                }
            }
        }
    }
    let mut out = Vec::with_capacity(lines.len());
    for line in &lines {
        if line.starts_with("m=") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 && parts[0] == "m=video" {
                let mut media = parts[..3].join(" ");
                for pt in &parts[3..] {
                    if !rtx.contains(*pt) {
                        media.push(' ');
                        media.push_str(pt);
                    }
                }
                out.push(media);
                continue;
            }
        }
        if let Some(pt) = attribute_pt(line) {
            if rtx.contains(pt) {
                continue;
            }
        }
        out.push(line.clone());
    }
    let mut text = out.join("\r\n");
    text.push_str("\r\n");
    text
}

fn attribute_pt(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("a=")?;
    let (_, value) = rest.split_once(':')?;
    let pt = value.split([' ', '/']).next()?;
    if !pt.is_empty() && pt.bytes().all(|byte| byte.is_ascii_digit()) {
        Some(pt)
    } else {
        None
    }
}

fn public_error(err: &impl ToString) -> String {
    let text = err.to_string().replace(['\r', '\n'], " ");
    let lower = text.to_ascii_lowercase();
    if text.len() > 180 || lower.contains("v=0") || lower.contains("a=candidate") || lower.contains("fingerprint") {
        "the screen answer was rejected".to_string()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_candidate_must_be_the_lan_address() {
        let lan = "192.168.1.8";
        assert!(host_on_lan("candidate:1 1 udp 1 192.168.1.8 50000 typ host", lan));
        assert!(!host_on_lan("candidate:1 1 udp 1 192.168.1.80 50000 typ host", lan));
        assert!(!host_on_lan("candidate:1 1 udp 1 10.0.0.8 50000 typ host", lan));
        assert!(!host_on_lan("candidate:1 1 tcp 1 192.168.1.8 9 typ host tcptype passive", lan));
        assert!(!host_on_lan("candidate:1 1 udp 1 192.168.1.8 50000 typ srflx raddr 1.2.3.4 rport 9", lan));
        assert!(!host_on_lan("candidate:1 1 udp 1 mac.local 50000 typ host", lan));
        assert!(!host_on_lan("a=candidate:1 1 udp 1 192.168.1.8 50000 typ host", ""));
    }

    #[tokio::test]
    async fn answer_names_the_phones_h264_profile() {
        let fingerprint = (0..32).map(|byte| format!("{byte:02X}")).collect::<Vec<_>>().join(":");
        let offer = format!(
            "v=0\r\no=- 0 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96 97 98\r\nc=IN IP4 0.0.0.0\r\na=rtcp:9 IN IP4 0.0.0.0\r\na=ice-ufrag:phon\r\na=ice-pwd:phonepasswordphonepassword\r\na=ice-options:trickle\r\na=fingerprint:sha-256 {fingerprint}\r\na=setup:actpass\r\na=mid:0\r\na=sendonly\r\na=rtcp-mux\r\na=rtpmap:96 H264/90000\r\na=fmtp:96 level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=420033\r\na=rtcp-fb:96 nack pli\r\na=rtpmap:97 rtx/90000\r\na=fmtp:97 apt=96\r\na=rtpmap:98 VP8/90000\r\n"
        );
        let filtered = without_rtx(&crate::desk::h264_only_sdp(&offer));
        assert!(filtered.contains("420033"));
        assert!(!filtered.to_ascii_lowercase().contains("vp8"));
        let pc = new_peer("127.0.0.1".parse().unwrap()).await.expect("peer");
        pc.set_remote_description(RTCSessionDescription::offer(filtered).unwrap()).await.expect("remote");
        let answer = pc.create_answer(None).await.expect("answer");
        let sdp = answer.sdp.to_ascii_lowercase();
        assert!(sdp.contains("profile-level-id=420033"), "{sdp}");
        assert!(sdp.contains("a=recvonly"), "{sdp}");
        assert!(!sdp.contains("vp8"), "{sdp}");
        assert!(!sdp.contains("h265"), "{sdp}");
        assert_eq!(sdp.matches("\nm=").count() + usize::from(sdp.starts_with("m=")), 1, "{sdp}");
        let _ = pc.close().await;
    }

    #[test]
    fn display_shrinks_the_long_side_and_keeps_the_shape() {
        assert_eq!(display_size(1220, 2712), (432, 960));
        assert_eq!(display_size(800, 400), (800, 400));
    }

    #[test]
    fn retransmission_payload_is_removed() {
        let sdp = "v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96 97\r\na=rtpmap:96 H264/90000\r\na=fmtp:96 profile-level-id=420033\r\na=rtpmap:97 rtx/90000\r\na=fmtp:97 apt=96\r\n";
        let filtered = without_rtx(sdp);
        assert!(filtered.contains("96"));
        assert!(!filtered.contains("rtx"));
        assert!(!filtered.contains(" 97"));
        assert!(filtered.contains("420033"));
    }
}
