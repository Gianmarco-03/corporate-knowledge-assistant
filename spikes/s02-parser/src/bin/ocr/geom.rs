//! Geometria px ↔ pt e DPI dal chunk `pHYs` (piano 01-13, TEST-8 e TEST-9).
//!
//! STUB del test-write (2026-10-23): corpi `todo!()` scritti dall'esecutore del Task 2.
//! Trasformazione diretta (render + degradazione): `px = x·dpi/72`, `py = (page_h_pt − y)·dpi/72`
//! (asse Y invertito: PDF in basso a sinistra, immagine in alto a sinistra), poi rotazione di
//! `rotation_deg` attorno a `center_px`. L'inversa la disfa nell'ordine opposto.

/// Trasformazione registrata in `<pagina>.transform.json` (più `page_h_pt` del render).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Transform {
    pub dpi: f64,
    pub width_px: u32,
    pub height_px: u32,
    pub page_h_pt: f64,
    pub rotation_deg: f64,
    pub center_px: (f64, f64),
}

/// Errore di lettura del DPI.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DpiError {
    /// Nessun `pHYs`, o unità non metro: l'esito del figlio è `failed/Corrupt` `dpi_missing`.
    Missing,
    /// Non è un PNG leggibile.
    NotPng,
}

/// DPI effettivo (arrotondato) dal chunk `pHYs` di un PNG, letto a mano dai chunk.
pub(crate) fn read_phys_dpi(_png: &[u8]) -> Result<u32, DpiError> {
    todo!("01-13 Task 2: read_phys_dpi")
}

/// Punto PDF (pt, origine in basso a sinistra) → pixel dell'immagine trasformata.
pub(crate) fn to_px(_t: &Transform, _pt: (f64, f64)) -> (f64, f64) {
    todo!("01-13 Task 2: to_px")
}

/// Pixel dell'immagine trasformata → punto PDF (inversa di [`to_px`]).
pub(crate) fn to_pt(_t: &Transform, _px: (f64, f64)) -> (f64, f64) {
    todo!("01-13 Task 2: to_pt")
}

/// Bbox in pixel `[left, top, right, bottom]` → rettangolo PDF `[x0, y0, x1, y1]` (involucro
/// dei quattro vertici riportati con [`to_pt`]).
pub(crate) fn map_bbox(_t: &Transform, _px: [f64; 4]) -> [f64; 4] {
    todo!("01-13 Task 2: map_bbox")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crc32(data: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }

    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut v = (data.len() as u32).to_be_bytes().to_vec();
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        v.extend_from_slice(&body);
        v.extend_from_slice(&crc32(&body).to_be_bytes());
        v
    }

    fn png(phys: Option<(u32, u8)>) -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = 1u32.to_be_bytes().to_vec();
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
        v.extend(chunk(b"IHDR", &ihdr));
        if let Some((ppm, unit)) = phys {
            let mut p = ppm.to_be_bytes().to_vec();
            p.extend_from_slice(&ppm.to_be_bytes());
            p.push(unit);
            v.extend(chunk(b"pHYs", &p));
        }
        v.extend(chunk(
            b"IDAT",
            &[0x78, 0x9c, 0x63, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01],
        ));
        v.extend(chunk(b"IEND", &[]));
        v
    }

    #[test]
    fn test8_phys_dpi_is_read_from_png_chunks() {
        assert_eq!(read_phys_dpi(&png(Some((11811, 1)))), Ok(300));
        assert_eq!(read_phys_dpi(&png(Some((7874, 1)))), Ok(200));
    }

    #[test]
    fn test8_phys_missing_or_unitless_is_dpi_missing() {
        assert_eq!(read_phys_dpi(&png(None)), Err(DpiError::Missing));
        assert_eq!(
            read_phys_dpi(&png(Some((11811, 0)))),
            Err(DpiError::Missing)
        );
        assert_eq!(read_phys_dpi(b"GIF89a....."), Err(DpiError::NotPng));
    }

    fn a4(dpi: f64, rot: f64) -> Transform {
        let (w, h) = ((595.0 * dpi / 72.0) as u32, (842.0 * dpi / 72.0) as u32);
        Transform {
            dpi,
            width_px: w,
            height_px: h,
            page_h_pt: 842.0,
            rotation_deg: rot,
            center_px: (w as f64 / 2.0, h as f64 / 2.0),
        }
    }

    fn close(a: (f64, f64), b: (f64, f64), tol: f64) -> bool {
        (a.0 - b.0).abs() <= tol && (a.1 - b.1).abs() <= tol
    }

    #[test]
    fn test9_forward_scale_and_y_axis_at_zero_rotation() {
        let t = a4(300.0, 0.0);
        assert!(close(to_px(&t, (0.0, 842.0)), (0.0, 0.0), 1e-9));
        assert!(close(to_px(&t, (72.0, 770.0)), (300.0, 300.0), 1e-9));
        assert!(close(to_pt(&t, (300.0, 300.0)), (72.0, 770.0), 1e-9));
        let t2 = a4(200.0, 0.0);
        assert!(close(
            to_px(&t2, (36.0, 842.0 - 36.0)),
            (100.0, 100.0),
            1e-9
        ));
    }

    #[test]
    fn test9_rotation_moves_far_corners_and_keeps_center() {
        let t = a4(200.0, 1.5);
        let c = t.center_px;
        let c_pt = to_pt(&a4(200.0, 0.0), c);
        assert!(
            close(to_px(&t, c_pt), c, 1e-6),
            "il centro di rotazione resta fermo"
        );
        let corner = to_px(&t, (0.0, 842.0));
        assert!(
            (corner.0.powi(2) + corner.1.powi(2)).sqrt() > 1.0,
            "rotazione applicata: {corner:?}"
        );
    }

    #[test]
    fn test9_analytic_stage1_round_trip_within_half_pixel() {
        // Stadio (1) del controllo analitico: rettangoli attesi → px (diretta) → pt (inversa),
        // errore massimo agli angoli ≤ 0,5 px equivalenti, a rotazione 0 e 1,5°.
        for (dpi, rot) in [(300.0, 0.0), (200.0, 0.0), (300.0, 1.5), (200.0, 1.5)] {
            let t = a4(dpi, rot);
            let tol_pt = 0.5 * 72.0 / dpi;
            for rect in [
                [20.0, 800.0, 110.0, 814.0],
                [480.0, 20.0, 575.0, 44.0],
                [250.0, 400.0, 330.0, 410.0],
            ] {
                for v in [
                    (rect[0], rect[1]),
                    (rect[2], rect[1]),
                    (rect[0], rect[3]),
                    (rect[2], rect[3]),
                ] {
                    let back = to_pt(&t, to_px(&t, v));
                    assert!(
                        close(back, v, tol_pt),
                        "dpi {dpi} rot {rot}: {v:?} → {back:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn test9_map_bbox_returns_pdf_rect_with_y_up() {
        let t = a4(300.0, 0.0);
        // Parola fra (300,300) e (510,338) px → x 72..122,4 pt; y dal basso 760,88..770 pt.
        let r = map_bbox(&t, [300.0, 300.0, 510.0, 338.0]);
        let want = [
            72.0,
            842.0 - 338.0 * 72.0 / 300.0,
            510.0 * 72.0 / 300.0,
            770.0,
        ];
        for i in 0..4 {
            assert!((r[i] - want[i]).abs() < 1e-9, "{r:?} vs {want:?}");
        }
        assert!(r[0] < r[2] && r[1] < r[3]);
    }
}
