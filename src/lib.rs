use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::copy;
use std::os::raw::c_char;
use std::ptr;
use regex::Regex;
use reqwest::blocking::Client;
use serde::Deserialize;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_MP3};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, ReadOnlySource};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

thread_local! {
    static LAST_ERROR: RefCell<String> = RefCell::new(String::new());
}

fn set_error(msg: &str) {
    LAST_ERROR.with(|e| *e.borrow_mut() = msg.to_string());
}

#[derive(Deserialize)]
struct TrackInfo {
    title: String,
    duration: u64,
    media: Media,
    user: User,
    #[serde(default)]
    permalink_url: Option<String>,
}

#[derive(Deserialize)]
struct User {
    username: String,
}

#[derive(Deserialize)]
struct Media {
    transcodings: Vec<Transcoding>,
}

#[derive(Deserialize)]
struct Transcoding {
    url: String,
    format: Format,
}

#[derive(Deserialize)]
struct Format {
    protocol: String,
}

#[derive(Deserialize)]
struct StreamUrl {
    url: String,
}

#[derive(Deserialize)]
struct SearchResponse {
    collection: Vec<TrackInfo>,
}

pub struct SearchContext {
    results: Vec<(CString, CString, u64)>, 
}

type PcmCallback = extern "C" fn(*const f32, u32);

fn get_client() -> Client {
    Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .build()
        .unwrap_or_default()
}

fn fetch_client_id(client: &Client) -> Option<String> {
    let start_url = "https://soundcloud.com/discover";
    let home = client.get(start_url).send().ok()?.text().ok()?;
    let re_script = Regex::new(r#"src="([^"]+\.js)""#).ok()?;
    let script_urls: Vec<String> = re_script.captures_iter(&home)
        .map(|cap| {
            let url = cap[1].to_string();
            if url.starts_with('/') {
                format!("https://soundcloud.com{}", url)
            } else {
                url
            }
        })
        .collect();

    let re_id = Regex::new(r#"client_id[:=]\s*["']?([a-zA-Z0-9]{32})["']?"#).ok()?;
    for url in script_urls.iter().rev().take(5) {
        if let Ok(js) = client.get(url).send().and_then(|r| r.text()) {
            if let Some(cap) = re_id.captures(&js) {
                return Some(cap[1].to_string());
            }
        }
    }
    None
}

#[no_mangle]
pub extern "C" fn sc_get_last_error() -> *mut c_char {
    LAST_ERROR.with(|e| {
        let s = e.borrow();
        if s.is_empty() {
            ptr::null_mut()
        } else {
            CString::new(s.as_str()).unwrap().into_raw()
        }
    })
}

#[no_mangle]
pub extern "C" fn sc_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe { let _ = CString::from_raw(s); }
    }
}

#[no_mangle]
pub extern "C" fn sc_search(c_query: *const c_char) -> *mut SearchContext {
    if c_query.is_null() { return ptr::null_mut(); }
    let query = unsafe { CStr::from_ptr(c_query).to_string_lossy() };
    let client = get_client();

    let client_id = match fetch_client_id(&client) {
        Some(id) => id,
        None => {
            set_error("Could not scrape client_id from SoundCloud");
            return ptr::null_mut();
        }
    };

    let url = format!("https://api-v2.soundcloud.com/search/tracks?q={}&client_id={}&limit=10", query, client_id);
    let resp: SearchResponse = match client.get(&url).send().and_then(|r| r.json()) {
        Ok(r) => r,
        Err(e) => {
            set_error(&format!("Search network error: {}", e));
            return ptr::null_mut();
        }
    };

    let mut results = Vec::new();
    for track in resp.collection {
        // Skip tracks that don't have progressive MP3 (usually Go+ tracks or HLS only)
        if track.media.transcodings.iter().any(|t| t.format.protocol == "progressive") {
            if let Some(p_url) = track.permalink_url {
                if let (Ok(t), Ok(u)) = (CString::new(format!("{} - {}", track.user.username, track.title)), CString::new(p_url)) {
                    results.push((t, u, track.duration));
                }
            }
        }
    }

    Box::into_raw(Box::new(SearchContext { results }))
}

#[no_mangle]
pub extern "C" fn sc_search_result_count(ctx: *mut SearchContext) -> u32 {
    if ctx.is_null() { return 0; }
    unsafe {
        let ctx_ref = &*ctx;
        ctx_ref.results.len() as u32
    }
}

#[no_mangle]
pub extern "C" fn sc_search_result_get_title(ctx: *mut SearchContext, idx: u32) -> *const c_char {
    if ctx.is_null() { return ptr::null(); }
    unsafe {
        let ctx_ref = &*ctx;
        ctx_ref.results.get(idx as usize).map(|x| x.0.as_ptr()).unwrap_or(ptr::null())
    }
}

#[no_mangle]
pub extern "C" fn sc_search_result_get_url(ctx: *mut SearchContext, idx: u32) -> *const c_char {
    if ctx.is_null() { return ptr::null(); }
    unsafe {
        let ctx_ref = &*ctx;
        ctx_ref.results.get(idx as usize).map(|x| x.1.as_ptr()).unwrap_or(ptr::null())
    }
}

#[no_mangle]
pub extern "C" fn sc_search_free(ctx: *mut SearchContext) {
    if !ctx.is_null() {
        unsafe { let _ = Box::from_raw(ctx); }
    }
}

#[no_mangle]
pub extern "C" fn sc_stream_track(
    c_url: *const c_char, 
    callback: PcmCallback, 
    stop_signal: *const bool
) -> i32 {
    if c_url.is_null() { return -1; }
    let url = unsafe { CStr::from_ptr(c_url).to_string_lossy() };
    let client = get_client();

    let client_id = match fetch_client_id(&client) {
        Some(id) => id,
        None => { set_error("Client ID not found during resolution"); return -2; }
    };

    let resolve_url = format!("https://api-v2.soundcloud.com/resolve?url={}&client_id={}", url, client_id);
    let resp = match client.get(&resolve_url).send() {
        Ok(r) => r,
        Err(e) => { set_error(&format!("Network error during resolve: {}", e)); return -3; }
    };

    if !resp.status().is_success() {
        set_error(&format!("SoundCloud API Error: HTTP {}", resp.status()));
        return -3;
    }

    let track: TrackInfo = match resp.json() {
        Ok(t) => t,
        Err(e) => { set_error(&format!("JSON Parse Error: {}", e)); return -3; }
    };

    let transcoding = match track.media.transcodings.iter().find(|t| t.format.protocol == "progressive") {
        Some(t) => t,
        None => { 
            set_error("Track format not supported (HLS/m3u8 only)"); 
            return -4; 
        }
    };

    let auth_stream_url = format!("{}?client_id={}", transcoding.url, client_id);
    let stream_obj: StreamUrl = match client.get(&auth_stream_url).send().and_then(|r| r.json()) {
        Ok(s) => s,
        Err(_) => { set_error("Failed to get stream URL"); return -5; }
    };

    let response = match client.get(&stream_obj.url).send() {
        Ok(r) => r,
        Err(_) => { set_error("Failed to connect to media stream"); return -6; }
    };

    let source = Box::new(ReadOnlySource::new(response));
    let mss = MediaSourceStream::new(source, Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");

    let probe = match symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default()) {
        Ok(p) => p,
        Err(_) => { set_error("Failed to probe audio format"); return -7; }
    };
    
    let mut format = probe.format;
    let track_id = format.tracks().iter().find(|t| t.codec_params.codec == CODEC_TYPE_MP3).unwrap().id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&format.tracks()[0].codec_params, &DecoderOptions::default()).unwrap();

    let mut interleaved = Vec::new();

    while let Ok(packet) = format.next_packet() {
        if !stop_signal.is_null() && unsafe { *stop_signal } {
            break;
        }
        if packet.track_id() != track_id { continue; }
        
        if let Ok(decoded) = decoder.decode(&packet) {
            if let AudioBufferRef::F32(buf) = decoded {
                let frames = buf.frames();
                let channels = buf.spec().channels.count();
                
                interleaved.clear();
                interleaved.reserve(frames * 2);

                if channels >= 2 {
                    let l = buf.chan(0);
                    let r = buf.chan(1);
                    for i in 0..frames {
                        interleaved.push(l[i]);
                        interleaved.push(r[i]);
                    }
                } else {
                    let l = buf.chan(0);
                    for i in 0..frames {
                        interleaved.push(l[i]);
                        interleaved.push(l[i]);
                    }
                }
                
                callback(interleaved.as_ptr(), interleaved.len() as u32);
            }
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn sc_download_track(c_url: *const c_char) -> i32 {
    if c_url.is_null() { return -1; }
    let url = unsafe { CStr::from_ptr(c_url).to_string_lossy() };
    let client = get_client();

    let client_id = match fetch_client_id(&client) {
        Some(id) => id,
        None => { set_error("Client ID not found"); return -2; }
    };

    let resolve_url = format!("https://api-v2.soundcloud.com/resolve?url={}&client_id={}", url, client_id);
    let resp = match client.get(&resolve_url).send() {
        Ok(r) => r,
        Err(e) => { set_error(&e.to_string()); return -3; }
    };

    if !resp.status().is_success() {
        set_error(&format!("Download HTTP Error: {}", resp.status()));
        return -3;
    }

    let track: TrackInfo = resp.json().unwrap();

    let transcoding = match track.media.transcodings.iter().find(|t| t.format.protocol == "progressive") {
        Some(t) => t,
        None => { set_error("No downloadable MP3 stream found"); return -4; }
    };

    let stream_url_api = format!("{}?client_id={}", transcoding.url, client_id);
    let stream_obj: StreamUrl = client.get(&stream_url_api).send().unwrap().json().unwrap();

    let filename = format!("{} - {}.mp3", track.user.username, track.title).replace("/", "_");
    let mut resp = client.get(&stream_obj.url).send().unwrap();
    let mut file = File::create(&filename).unwrap();

    copy(&mut resp, &mut file).unwrap();
    0
}