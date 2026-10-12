//! The engine over the real models, where the app's assets hold them:
//! both graphs load and run under `tract` at the camera's geometry, a
//! blank frame has no face, and an embedding has the dimension the
//! template needs. Skipped, saying so, where the assets are absent.

use rhtn_engine::{DIM, Face, SFace, Thresholds};

fn models() -> Option<(Vec<u8>, Vec<u8>)> {
    let root = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../mobile/android/app/src/main/assets/"
    );
    let y = std::fs::read(format!("{root}yunet.onnx")).ok()?;
    let s = std::fs::read(format!("{root}sface.onnx")).ok()?;
    Some((y, s))
}

#[test]
fn both_models_load_and_run_at_the_cameras_geometry() {
    let Some((y, s)) = models() else {
        eprintln!("models absent: skipped");
        return;
    };
    let engine = SFace::from_onnx(&y, &s, 640, 480, Thresholds::default()).expect("both load");
    assert!(engine.find(&[]).is_none(), "an empty frame is nobody");
    let blank = vec![0u8; 640 * 480];
    let t0 = std::time::Instant::now();
    assert!(engine.detect(&blank).is_none(), "nobody in a black frame");
    eprintln!("detect: {} ms on this host", t0.elapsed().as_millis());
    let mid = vec![128u8; 640 * 480];
    assert!(engine.detect(&mid).is_none(), "nobody in a grey one");
    // an embedding of whatever is at a plausible landmark set has the
    // template's dimension and is a unit vector
    let face = Face {
        score: 1.0,
        x: 200.0,
        y: 100.0,
        w: 150.0,
        h: 150.0,
        landmarks: [
            (250.0, 150.0),
            (300.0, 150.0),
            (275.0, 180.0),
            (255.0, 210.0),
            (295.0, 210.0),
        ],
    };
    let e = engine.embed(&mid, &face).expect("runs");
    assert_eq!(e.len(), DIM);
    let n: f32 = e.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (n - 1.0).abs() < 1e-3 || n == 0.0,
        "unit, or zero where the model saw nothing: {n}"
    );
}
