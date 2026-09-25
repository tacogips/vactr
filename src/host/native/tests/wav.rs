//! The WAV decoder and the loader's root confinement (17).

use std::path::Path;
use std::rc::Rc;

use super::TempDir;
use crate::host::caps::{SampleLoader, SampleSrc};
use crate::host::native::{parse_wav, NativeSampleLoader};
use crate::ns::load::SourceLoader;
use crate::reader::span::FileId;
use crate::value::intern::intern_kw;
use crate::value::value::PathVal;
use crate::vm::fail::FailCode;

/// A WAV file: `tag` 1 (PCM) or 3 (float), `bits`, `channels`, raw data.
fn wav(tag: u16, bits: u16, channels: u16, rate: u32, data: &[u8]) -> Vec<u8> {
    let align = channels * bits / 8;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    b.extend_from_slice(b"WAVE");
    b.extend_from_slice(b"fmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&tag.to_le_bytes());
    b.extend_from_slice(&channels.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * u32::from(align)).to_le_bytes());
    b.extend_from_slice(&align.to_le_bytes());
    b.extend_from_slice(&bits.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(data.len() as u32).to_le_bytes());
    b.extend_from_slice(data);
    b
}

fn i16s(v: &[i16]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

#[test]
fn pcm16_mono() {
    let d = parse_wav(&wav(1, 16, 1, 44_100, &i16s(&[0, 16_384, -32_768]))).unwrap();
    assert_eq!((d.rate, d.channels), (44_100, 1));
    assert_eq!(&*d.frames, &[0.0, 0.5, -1.0]);
}

#[test]
fn pcm16_stereo_is_interleaved() {
    let d = parse_wav(&wav(1, 16, 2, 48_000, &i16s(&[16_384, -16_384, 0, 8_192]))).unwrap();
    assert_eq!(d.channels, 2);
    assert_eq!(&*d.frames, &[0.5, -0.5, 0.0, 0.25]);
}

#[test]
fn pcm24() {
    // 0x400000 = 0.5, 0xC00000 = -0.5, 0x7FFFFF ~ 1.
    let data = [0x00, 0x00, 0x40, 0x00, 0x00, 0xC0, 0xFF, 0xFF, 0x7F];
    let d = parse_wav(&wav(1, 24, 1, 48_000, &data)).unwrap();
    assert_eq!(d.frames[0], 0.5);
    assert_eq!(d.frames[1], -0.5);
    assert!((d.frames[2] - 1.0).abs() < 1e-6);
}

#[test]
fn pcm32_stereo() {
    let data: Vec<u8> = [1 << 30, -(1 << 30)]
        .iter()
        .flat_map(|x: &i32| x.to_le_bytes())
        .collect();
    let d = parse_wav(&wav(1, 32, 2, 96_000, &data)).unwrap();
    assert_eq!((d.rate, d.channels), (96_000, 2));
    assert_eq!(&*d.frames, &[0.5, -0.5]);
}

#[test]
fn float32_mono_and_stereo() {
    let data: Vec<u8> = [0.25f32, -0.75]
        .iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    let mono = parse_wav(&wav(3, 32, 1, 48_000, &data)).unwrap();
    assert_eq!(&*mono.frames, &[0.25, -0.75]);
    let stereo = parse_wav(&wav(3, 32, 2, 48_000, &data)).unwrap();
    assert_eq!(stereo.channels, 2);
    assert_eq!(stereo.frames.len(), 2);
}

#[test]
fn chunks_before_data_are_skipped_with_padding() {
    let mut b = wav(1, 16, 1, 48_000, &i16s(&[16_384]));
    // Insert an odd-sized LIST chunk (padded to even) before `data`.
    let at = 36;
    let list = [b"LIST".as_slice(), &3u32.to_le_bytes(), b"abc\0"].concat();
    b.splice(at..at, list);
    let d = parse_wav(&b).unwrap();
    assert_eq!(&*d.frames, &[0.5]);
}

#[test]
fn truncated_and_garbage_input_fail_without_panicking() {
    let good = wav(1, 16, 2, 48_000, &i16s(&[1, 2, 3, 4]));
    for n in 0..good.len() {
        assert!(parse_wav(&good[..n]).is_err(), "prefix of {n} bytes");
    }
    assert!(parse_wav(b"not a wav file at all").is_err());
    let mut seed = 7u32;
    for _ in 0..200 {
        let mut junk = good.clone();
        for b in junk.iter_mut().skip(12) {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            if seed >> 29 == 0 {
                *b = (seed >> 8) as u8;
            }
        }
        let _ = parse_wav(&junk);
    }
}

#[test]
fn unsupported_formats_are_refused() {
    assert!(parse_wav(&wav(1, 8, 1, 48_000, &[0, 1])).is_err());
    assert!(parse_wav(&wav(3, 64, 1, 48_000, &[0; 8])).is_err());
    assert!(parse_wav(&wav(2, 16, 1, 48_000, &[0; 2])).is_err());
    assert!(parse_wav(&wav(1, 16, 6, 48_000, &[0; 12])).is_err());
}

fn path(text: &str, file: Option<FileId>) -> PathVal {
    PathVal {
        text: Rc::from(text),
        file,
    }
}

fn write(p: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

#[test]
fn loads_a_path_inside_the_root() {
    let dir = TempDir::new("root");
    write(
        &dir.path().join("kick.wav"),
        &wav(1, 16, 1, 44_100, &i16s(&[16_384])),
    );
    let mut l = NativeSampleLoader::new(&[dir.path().to_path_buf()], dir.path());
    let d = l.load(&SampleSrc::Path(path("./kick.wav", None))).unwrap();
    assert_eq!((d.rate, &*d.frames), (44_100, &[0.5][..]));
}

#[test]
fn a_path_outside_the_roots_is_rejected() {
    let outside = TempDir::new("outside");
    let root = TempDir::new("inside");
    let file = outside.path().join("x.wav");
    write(&file, &wav(1, 16, 1, 48_000, &i16s(&[1])));
    let mut l = NativeSampleLoader::new(&[root.path().to_path_buf()], root.path());
    let abs = file.to_string_lossy().to_string();
    let err = l.load(&SampleSrc::Path(path(&abs, None))).unwrap_err();
    assert_eq!(err.code, FailCode::HostUnavailable);
    assert!(err.message.contains("outside"), "{}", err.message);
    // `..` out of the root is rejected the same way.
    let rel = format!(
        "../{}/x.wav",
        outside.path().file_name().unwrap().to_string_lossy()
    );
    let err = l.load(&SampleSrc::Path(path(&rel, None))).unwrap_err();
    assert!(err.message.contains("outside"), "{}", err.message);
}

#[test]
fn a_missing_or_malformed_file_is_a_failure() {
    let dir = TempDir::new("bad");
    write(&dir.path().join("bad.wav"), b"RIFF....WAVEjunk");
    let mut l = NativeSampleLoader::new(&[dir.path().to_path_buf()], dir.path());
    for p in ["./missing.wav", "./bad.wav"] {
        let err = l.load(&SampleSrc::Path(path(p, None))).unwrap_err();
        assert_eq!(err.code, FailCode::HostUnavailable, "{p}");
    }
}

#[test]
fn a_bank_index_picks_the_sorted_wav_modulo_count() {
    let dir = TempDir::new("bank");
    for (name, v) in [("b.wav", 8_192i16), ("a.wav", 16_384), ("c.txt", 0)] {
        write(
            &dir.path().join("bd").join(name),
            &wav(1, 16, 1, 48_000, &i16s(&[v])),
        );
    }
    let mut l = NativeSampleLoader::new(&[dir.path().to_path_buf()], dir.path());
    let kw = intern_kw("bd");
    let first = l.load(&SampleSrc::Bank { kw, index: 0 }).unwrap();
    let second = l.load(&SampleSrc::Bank { kw, index: 1 }).unwrap();
    let wrapped = l.load(&SampleSrc::Bank { kw, index: 2 }).unwrap();
    assert_eq!(first.frames[0], 0.5);
    assert_eq!(second.frames[0], 0.25);
    assert_eq!(wrapped.frames[0], 0.5);
    assert!(l
        .load(&SampleSrc::Bank {
            kw: intern_kw("nope"),
            index: 0
        })
        .is_err());
}

#[test]
fn relative_paths_resolve_against_the_loaded_file() {
    let dir = TempDir::new("rel");
    write(
        &dir.path().join("songs/kit/snare.wav"),
        &wav(1, 16, 1, 48_000, &i16s(&[16_384])),
    );
    write(&dir.path().join("songs/song.vact"), b"1 + 1\n");
    let mut l = NativeSampleLoader::new(&[dir.path().to_path_buf()], dir.path());
    let (file, text) = l.read(&path("./songs/song.vact", None)).unwrap();
    assert_eq!(&*text, "1 + 1\n");
    let d = l
        .load(&SampleSrc::Path(path("./kit/snare.wav", Some(file))))
        .unwrap();
    assert_eq!(d.frames[0], 0.5);
}
