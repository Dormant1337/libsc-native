use std::ffi::CStr;
use std::fs::File;
use std::io::copy;
use std::os::raw::c_char;
use regex::Regex;
use reqwest::blocking::Client;
use serde::Deserialize;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_MP3};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, ReadOnlySource};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[derive(Deserialize)]
struct TrackInfo {
    title: String,
    media: Media,
    user: User,
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

type PcmCallback = extern "C" fn(*const f32, u32);

fn get_client() -> Client {
    Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .build()
        .unwrap_or_default()
}

fn fetch_client_id(client: &Client, track_url: &str) -> Option<String> {
    let home = client.get(track_url).send().ok()?.text().ok()?;
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
    for url in script_urls.iter().rev().take(10) {
        if let Ok(js) = client.get(url).send().and_then(|r| r.text()) {
            if let Some(cap) = re_id.captures(&js) {
                return Some(cap[1].to_string());
            }
        }
    }
    None
}

fn get_stream_url(client: &Client, track_url: &str) -> Option<(String, String)> {
    let client_id = fetch_client_id(client, track_url)?;
    let resolve_url = format!(
        "https://api-v2.soundcloud.com/resolve?url={}&client_id={}",
        track_url, client_id
    );
    let track: TrackInfo = client.get(&resolve_url).send().ok()?.json().ok()?;
    let transcoding = track.media.transcodings.iter()
        .find(|t| t.format.protocol == "progressive")?;
    
    let auth_stream_url = format!("{}?client_id={}", transcoding.url, client_id);
    let stream_obj: StreamUrl = client.get(&auth_stream_url).send().ok()?.json().ok()?;
    
    let filename = format!("{} - {}.mp3", track.user.username, track.title)
        .replace("/", "_");

    Some((stream_obj.url, filename))
}

#[no_mangle]
pub extern "C" fn sc_stream_track(c_url: *const c_char, callback: PcmCallback) -> i32 {
    if c_url.is_null() { return -1; }
    let url = unsafe { CStr::from_ptr(c_url).to_string_lossy() };
    let client = get_client();

    let (stream_url, _) = match get_stream_url(&client, &url) {
        Some(u) => u,
        None => return -2,
    };

    let response = match client.get(&stream_url).send() {
        Ok(r) => r,
        Err(_) => return -3,
    };

    let source = Box::new(ReadOnlySource::new(response));
    let mss = MediaSourceStream::new(source, Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");

    let probe = match symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default()) {
            Ok(p) => p,
            Err(_) => return -4,
        };

    let mut format = probe.format;
    let track = match format.tracks().iter().find(|t| t.codec_params.codec == CODEC_TYPE_MP3) {
        Some(t) => t,
        None => return -5,
    };
    
    let mut decoder = match symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default()) {
            Ok(d) => d,
            Err(_) => return -6,
        };

    let track_id = track.id;

    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id { continue; }
        if let Ok(decoded) = decoder.decode(&packet) {
            if let AudioBufferRef::F32(buf) = decoded {
                callback(buf.chan(0).as_ptr(), buf.frames() as u32);
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

    let (stream_url, filename) = match get_stream_url(&client, &url) {
        Some(res) => res,
        None => return -2,
    };

    let mut response = match client.get(&stream_url).send() {
        Ok(r) => r,
        Err(_) => return -3,
    };

    let mut file = match File::create(&filename) {
        Ok(f) => f,
        Err(_) => return -4,
    };

    match copy(&mut response, &mut file) {
        Ok(_) => 0,
        Err(_) => -5,
    }
}