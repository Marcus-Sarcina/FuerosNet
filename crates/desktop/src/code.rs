//! **The code a desktop shows**, drawn as the phone reads one: the bytes
//! in base45 (RFC 9285), whose alphabet is exactly a QR's alphanumeric
//! set, at error correction M with a quiet zone of two modules — the
//! phone's `Optical` and `QrCamera`, which this must agree with to the
//! character.  How an object goes on a screen is the shell's and not the
//! wire format's (`wire-format.md` §14.3 fixes the bytes, not the symbol).

use qrcode::{EcLevel, QrCode};

const A: &[u8; 45] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";

/// The quiet zone, in modules each side: the phone's `Optical.MARGIN`.
pub const MARGIN: usize = 2;

/// `bytes` in base45: two bytes to three characters, little-endian base
/// 45, and one byte left over to two.
pub fn base45(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(2) * 3);
    for pair in bytes.chunks(2) {
        let (mut n, digits) = match pair {
            [a, b] => ((usize::from(*a) << 8) | usize::from(*b), 3),
            [a] => (usize::from(*a), 2),
            _ => unreachable!("chunks of two"),
        };
        for _ in 0..digits {
            out.push(A[n % 45] as char);
            n /= 45;
        }
    }
    out
}

/// **The symbol for `bytes` as an SVG**, black on white whatever the
/// window's mode: a camera reads it, and a code has no palette.  Its side
/// is `side` CSS pixels; the modules scale into it.
pub fn svg(bytes: &[u8], side: u32) -> Result<String, String> {
    let code = QrCode::with_error_correction_level(base45(bytes).as_bytes(), EcLevel::M)
        .map_err(|e| format!("the code does not fit a symbol: {e}"))?;
    let w = code.width();
    let total = w + 2 * MARGIN;
    let mut path = String::new();
    for (i, c) in code.to_colors().iter().enumerate() {
        if *c == qrcode::Color::Dark {
            let (x, y) = (i % w + MARGIN, i / w + MARGIN);
            path.push_str(&format!("M{x} {y}h1v1h-1z"));
        }
    }
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {total} {total}\" \
         width=\"{side}\" height=\"{side}\" shape-rendering=\"crispEdges\">\
         <rect width=\"{total}\" height=\"{total}\" fill=\"#fff\"/>\
         <path d=\"{path}\" fill=\"#000\"/></svg>"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base45_is_rfc_9285s() {
        // the RFC's own examples (§4.3)
        assert_eq!(base45(b"AB"), "BB8");
        assert_eq!(base45(b"Hello!!"), "%69 VD92EX0");
        assert_eq!(base45(b"base-45"), "UJCLQE7W581");
        assert_eq!(base45(b"ietf!"), "QED8WEX0");
        assert_eq!(base45(b""), "");
    }

    #[test]
    fn a_device_offer_draws_as_one_symbol_black_on_white() {
        // an offer's size: version, a 32-byte key, a 32-byte secret, an
        // address
        let offer = vec![0xa5u8; 4 + 34 + 34 + 24];
        let s = svg(&offer, 320).unwrap();
        assert!(s.starts_with("<svg"));
        assert!(s.contains("fill=\"#fff\"") && s.contains("fill=\"#000\""));
        assert!(s.contains("width=\"320\""));
    }
}
