pub(crate) fn lossless_gray(precision: u8) -> Vec<u8> {
    // T.81 lossless SOF3, predictor 1 and category-zero difference. The
    // first sample is the precision-defined midpoint, which exposes exact
    // display rounding for every admitted precision.
    let mut bytes = vec![
        0xff, 0xd8, 0xff, 0xc3, 0, 11, precision, 0, 1, 0, 1, 1, 1, 0x11, 0,
    ];
    let mut table = vec![0];
    table.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    table.push(0);
    push_segment(&mut bytes, 0xc4, &table);
    push_segment(&mut bytes, 0xda, &[1, 1, 0, 1, 0, 0]);
    bytes.extend_from_slice(&[0x7f, 0xff, 0xd9]);
    bytes
}

pub(crate) fn direct_rgb_twelve() -> Vec<u8> {
    // Independent direct-RGB 12-bit DCT fixture: the reconstructed channels
    // are exactly 0, 2048 and 4095 for every output pixel.
    let mut bytes = vec![0xff, 0xd8];
    let mut quantization = vec![0x10];
    for _ in 0..64 {
        quantization.extend_from_slice(&1_u16.to_be_bytes());
    }
    push_segment(&mut bytes, 0xdb, &quantization);
    push_segment(
        &mut bytes,
        0xc1,
        &[
            12, 0, 8, 0, 8, 3, b'R', 0x11, 0, b'G', 0x11, 0, b'B', 0x11, 0,
        ],
    );
    let mut huffman = vec![0x00];
    huffman.extend_from_slice(&[0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    huffman.extend_from_slice(&[0, 14, 15]);
    huffman.push(0x10);
    huffman.extend_from_slice(&[0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    huffman.extend_from_slice(&[0, 0x0e]);
    push_segment(&mut bytes, 0xc4, &huffman);
    push_segment(&mut bytes, 0xda, &[3, b'R', 0, b'G', 0, b'B', 0, 0, 63, 0]);
    bytes.extend_from_slice(&[0x9f, 0xff, 0, 0x80, 0xff, 0, 0xf0, 0x7f, 0xff, 0xd9]);
    bytes
}

fn push_segment(bytes: &mut Vec<u8>, marker: u8, payload: &[u8]) {
    bytes.extend_from_slice(&[0xff, marker]);
    let length = u16::try_from(payload.len() + 2).expect("fixture segment fits u16");
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(payload);
}
