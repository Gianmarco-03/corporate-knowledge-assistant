//! Trasformazione pura dei rettangoli PDF (piano 01-09, Task 1).
//!
//! Rettangoli `[x0, y0, x1, y1]` in punti. Funzione pura: nessun PDFium, nessuna metrica di
//! font; un errore della bbox di un glifo non la tocca mai. La viewport è la CropBox (la
//! regione visibile della pagina, PDF 32000 §14.11.2); la MediaBox resta nella firma perché
//! la CropBox efficace è la sua intersezione con la MediaBox: le pagine misurate hanno la
//! CropBox dentro la MediaBox, e qui la si interseca comunque.

/// `/Rotate` normalizzato in `{0, 90, 180, 270}`. Un valore non multiplo di 90 vale 0,
/// come in PDF.js, che lo ignora.
fn norm_rotate(rotate: i32) -> i32 {
    match rotate.rem_euclid(360) {
        r @ (90 | 180 | 270) => r,
        _ => 0,
    }
}

/// CropBox efficace: intersezione normalizzata di CropBox e MediaBox.
fn effective_crop(mediabox: [f64; 4], cropbox: [f64; 4]) -> [f64; 4] {
    let m = normalize(mediabox);
    let c = normalize(cropbox);
    [
        m[0].max(c[0]),
        m[1].max(c[1]),
        m[2].min(c[2]),
        m[3].min(c[3]),
    ]
}

fn normalize(r: [f64; 4]) -> [f64; 4] {
    [
        r[0].min(r[2]),
        r[1].min(r[3]),
        r[0].max(r[2]),
        r[1].max(r[3]),
    ]
}

/// Da spazio utente PDF (origine in basso a sinistra, prima di `/Rotate`) a viewport non
/// scalata con origine in alto a sinistra, come PDF.js `convertToViewportRectangle` a scala
/// 1; risultato normalizzato `x0 < x1`, `y0 < y1`. `rotate` multiplo di 90, anche negativo
/// o ≥ 360 (normalizzato modulo 360).
///
/// Passi per ogni vertice: (1) traslazione nell'origine della CropBox; (2) ribaltamento
/// dell'asse y (`v = H - y'`); (3) rotazione oraria della pagina: 0 → `(u, v)`, 90 →
/// `(H - v, u)`, 180 → `(W - u, H - v)`, 270 → `(v, W - u)`; (4) normalizzazione.
pub(crate) fn to_viewport(
    rect: [f64; 4],
    mediabox: [f64; 4],
    cropbox: [f64; 4],
    rotate: i32,
) -> [f64; 4] {
    let crop = effective_crop(mediabox, cropbox);
    let (w, h) = (crop[2] - crop[0], crop[3] - crop[1]);
    let rot = norm_rotate(rotate);
    let point = |x: f64, y: f64| {
        let u = x - crop[0];
        let v = h - (y - crop[1]);
        match rot {
            90 => (h - v, u),
            180 => (w - u, h - v),
            270 => (v, w - u),
            _ => (u, v),
        }
    };
    let (ax, ay) = point(rect[0], rect[1]);
    let (bx, by) = point(rect[2], rect[3]);
    [ax.min(bx), ay.min(by), ax.max(bx), ay.max(by)]
}

/// Larghezza e altezza della viewport (scambiate a 90 e 270).
pub(crate) fn viewport_size(mediabox: [f64; 4], cropbox: [f64; 4], rotate: i32) -> (f64, f64) {
    let crop = effective_crop(mediabox, cropbox);
    let (w, h) = (crop[2] - crop[0], crop[3] - crop[1]);
    if norm_rotate(rotate) % 180 == 0 {
        (w, h)
    } else {
        (h, w)
    }
}

#[cfg(test)]
mod tests {
    //! TEST-5: gli 8 casi della fixture a mano `rect_cases.expected.json` (unica fonte dei
    //! valori attesi, nessuna costante attesa qui), più proprietà che non usano valori attesi.
    use super::*;

    const RECT_FIXTURE: &str = include_str!("../../../locator-gen/rect_cases.expected.json");
    const TOL_PT: f64 = 0.5;

    struct Case {
        rotate: i32,
        mediabox: [f64; 4],
        cropbox: [f64; 4],
        user_rect: [f64; 4],
        expected: [f64; 4],
    }

    fn arr(v: &serde_json::Value) -> [f64; 4] {
        let a = v.as_array().expect("array di 4 numeri");
        assert_eq!(a.len(), 4);
        [
            a[0].as_f64().unwrap(),
            a[1].as_f64().unwrap(),
            a[2].as_f64().unwrap(),
            a[3].as_f64().unwrap(),
        ]
    }

    fn fixture() -> serde_json::Value {
        serde_json::from_str(RECT_FIXTURE).expect("fixture JSON")
    }

    fn cases() -> Vec<(String, Case)> {
        fixture()["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                (
                    c["case_id"].as_str().unwrap().to_owned(),
                    Case {
                        rotate: c["rotate"].as_i64().unwrap() as i32,
                        mediabox: arr(&c["mediabox"]),
                        cropbox: arr(&c["cropbox"]),
                        user_rect: arr(&c["user_rect"]),
                        expected: arr(&c["expected_viewport_rect"]),
                    },
                )
            })
            .collect()
    }

    fn case(id: &str) -> Case {
        cases()
            .into_iter()
            .find(|(i, _)| i == id)
            .map(|(_, c)| c)
            .unwrap_or_else(|| panic!("caso {id} assente dalla fixture"))
    }

    fn assert_close(got: [f64; 4], want: [f64; 4], id: &str) {
        let err = got
            .iter()
            .zip(want.iter())
            .map(|(g, w)| (g - w).abs())
            .fold(0.0, f64::max);
        assert!(
            err <= TOL_PT,
            "{id}: got {got:?}, atteso {want:?}, errore {err} pt"
        );
    }

    fn check(id: &str) {
        let c = case(id);
        assert_close(
            to_viewport(c.user_rect, c.mediabox, c.cropbox, c.rotate),
            c.expected,
            id,
        );
    }

    /// TEST-6: la fixture dei rettangoli è quella a mano (8 casi, derivazioni non vuote,
    /// 4 rotazioni × 2 CropBox, rettangolo asimmetrico).
    #[test]
    fn rect_fixture_is_hand_written_with_eight_cases() {
        let f = fixture();
        assert_eq!(f["class"], "derivazione-documentata");
        assert_eq!(f["author"], "test-write");
        let cs = f["cases"].as_array().unwrap();
        assert_eq!(cs.len(), 8);
        let mut combos: Vec<(i64, bool)> = cs
            .iter()
            .map(|c| {
                assert!(!c["derivation"].as_str().unwrap_or("").is_empty());
                (c["rotate"].as_i64().unwrap(), c["cropbox"] != c["mediabox"])
            })
            .collect();
        combos.sort_unstable();
        combos.dedup();
        assert_eq!(combos.len(), 8, "4 rotazioni × 2 condizioni di CropBox");
        for c in cs {
            let r = arr(&c["user_rect"]);
            assert_ne!(r[2] - r[0], r[3] - r[1], "rettangolo asimmetrico");
        }
    }

    #[test]
    fn rot0_cropbox_equal() {
        check("rot0-crop-eq");
    }

    #[test]
    fn rot90_cropbox_equal() {
        check("rot90-crop-eq");
    }

    #[test]
    fn rot180_cropbox_equal() {
        check("rot180-crop-eq");
    }

    #[test]
    fn rot270_cropbox_equal() {
        check("rot270-crop-eq");
    }

    #[test]
    fn rot0_cropbox_shifted() {
        check("rot0-crop-shift");
    }

    #[test]
    fn rot90_cropbox_shifted() {
        check("rot90-crop-shift");
    }

    #[test]
    fn rot180_cropbox_shifted() {
        check("rot180-crop-shift");
    }

    #[test]
    fn rot270_cropbox_shifted() {
        check("rot270-crop-shift");
    }

    /// Proprietà: risultato normalizzato, dentro la viewport, dimensioni conservate (scambiate
    /// a 90/270), viewport scambiata a 90/270.
    #[test]
    fn transform_preserves_size_and_stays_in_viewport() {
        for (id, c) in cases() {
            let g = to_viewport(c.user_rect, c.mediabox, c.cropbox, c.rotate);
            assert!(g[0] < g[2] && g[1] < g[3], "{id}: non normalizzato {g:?}");
            let (vw, vh) = viewport_size(c.mediabox, c.cropbox, c.rotate);
            assert!(
                g[0] >= -TOL_PT && g[1] >= -TOL_PT && g[2] <= vw + TOL_PT && g[3] <= vh + TOL_PT,
                "{id}: {g:?} fuori dalla viewport {vw}×{vh}"
            );
            let (w, h) = (
                c.user_rect[2] - c.user_rect[0],
                c.user_rect[3] - c.user_rect[1],
            );
            let (cw, ch) = (c.cropbox[2] - c.cropbox[0], c.cropbox[3] - c.cropbox[1]);
            let swapped = c.rotate % 180 != 0;
            let (ew, eh, evw, evh) = if swapped {
                (h, w, ch, cw)
            } else {
                (w, h, cw, ch)
            };
            assert!(((g[2] - g[0]) - ew).abs() <= TOL_PT, "{id}: larghezza");
            assert!(((g[3] - g[1]) - eh).abs() <= TOL_PT, "{id}: altezza");
            assert!(
                (vw - evw).abs() <= TOL_PT && (vh - evh).abs() <= TOL_PT,
                "{id}: viewport"
            );
        }
    }

    /// `/Rotate` fuori da 0..360 si normalizza: -90 ≡ 270, 450 ≡ 90, 360 ≡ 0.
    #[test]
    fn rotate_is_normalized_modulo_360() {
        for (_, c) in cases() {
            let base = to_viewport(c.user_rect, c.mediabox, c.cropbox, c.rotate);
            for alt in [c.rotate - 360, c.rotate + 360] {
                assert_close(
                    to_viewport(c.user_rect, c.mediabox, c.cropbox, alt),
                    base,
                    &format!("rotate {alt}"),
                );
            }
        }
    }

    /// Un rettangolo con estremi invertiti dà lo stesso risultato del normalizzato.
    #[test]
    fn inverted_input_rect_is_normalized() {
        for (id, c) in cases() {
            let r = c.user_rect;
            let inv = [r[2], r[3], r[0], r[1]];
            assert_close(
                to_viewport(inv, c.mediabox, c.cropbox, c.rotate),
                to_viewport(r, c.mediabox, c.cropbox, c.rotate),
                &id,
            );
        }
    }
}
