//! A measurement, not a test of the suite: run the detector over frames
//! named by `RHTN_FRAMES` (640×480 luminance, one file each) at both
//! canvases and print what each finds.  Ignored unless asked for.
use rhtn_engine::{SFace, Thresholds};

#[test]
#[ignore]
fn measure_canvases() {
    let root = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../mobile/android/app/src/main/assets/"
    );
    let y = std::fs::read(format!("{root}yunet.onnx")).unwrap();
    let s = std::fs::read(format!("{root}sface.onnx")).unwrap();
    let frames: Vec<String> = std::env::var("RHTN_FRAMES")
        .unwrap_or_default()
        .split(':')
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect();
    for canvas in [640usize, 320] {
        let e = SFace::from_onnx_at(&y, &s, 640, 480, Thresholds::default(), canvas).unwrap();
        for f in &frames {
            let g = std::fs::read(f).unwrap();
            let t0 = std::time::Instant::now();
            let found = e.find_scored(&g);
            let ms = t0.elapsed().as_millis();
            match found {
                Some((face, turns)) => println!(
                    "canvas {canvas} {f}: face score {:.3} w {:.0} h {:.0} at ({:.0},{:.0}) turns {turns} in {ms} ms",
                    face.score, face.w, face.h, face.x, face.y
                ),
                None => println!("canvas {canvas} {f}: no face in {ms} ms"),
            }
        }
    }
}
