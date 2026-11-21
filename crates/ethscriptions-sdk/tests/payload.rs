use std::io::Cursor;

use ethscriptions_sdk::digest::compute_digest;
use ethscriptions_sdk::encode::{
    DataUriOptions, InputSource, PrepareInscribeOptions, build_data_uri, build_data_uri_with_type,
    gzip_bytes, optimize_image, parse_data_uri, prepare_payload,
};

const PNG_MAGIC: [u8; 16] = [
    0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D, b'I', b'H', b'D', b'R',
];

fn uri(bytes: &[u8], skip_content_type: bool, esip6: bool) -> String {
    build_data_uri(
        bytes,
        &DataUriOptions {
            skip_content_type,
            esip6,
        },
    )
}

fn error_text(uri: &str) -> String {
    parse_data_uri(uri).unwrap_err().to_string()
}

fn png() -> Vec<u8> {
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        3,
        2,
        image::Rgba([200, 30, 90, 255]),
    ));
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

#[test]
fn parse_reverses_build_for_text() {
    let uri = uri(b"hello world!", false, false);
    assert_eq!(uri, "data:text/plain,hello%20world%21");
    let (bytes, content_type) = parse_data_uri(&uri).unwrap();
    assert_eq!(bytes, b"hello world!");
    assert_eq!(content_type, "text/plain");
}

#[test]
fn parse_reverses_build_for_json_and_svg() {
    let json = br#"{"name":"test","list":[1,2,3]}"#;
    let (bytes, content_type) = parse_data_uri(&uri(json, false, false)).unwrap();
    assert_eq!(bytes, json);
    assert_eq!(content_type, "application/json");

    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#;
    let (bytes, content_type) = parse_data_uri(&uri(svg, false, false)).unwrap();
    assert_eq!(bytes, svg);
    assert_eq!(content_type, "image/svg+xml");
}

#[test]
fn parse_reverses_build_for_binary() {
    let uri = uri(&PNG_MAGIC, false, false);
    assert!(uri.starts_with("data:image/png;base64,"));
    let (bytes, content_type) = parse_data_uri(&uri).unwrap();
    assert_eq!(bytes, PNG_MAGIC);
    assert_eq!(content_type, "image/png");
}

#[test]
fn parse_reverses_build_for_every_byte_value() {
    let all: Vec<u8> = (0..=255).collect();
    let (bytes, content_type) = parse_data_uri(&uri(&all, false, false)).unwrap();
    assert_eq!(bytes, all);
    assert_eq!(content_type, "application/octet-stream");

    let ascii: Vec<u8> = (0..128).collect();
    let (bytes, content_type) = parse_data_uri(&uri(&ascii, false, false)).unwrap();
    assert_eq!(bytes, ascii);
    assert_eq!(content_type, "text/plain");
}

#[test]
fn parse_keeps_esip6_in_the_content_type() {
    let text = uri(b"hello", false, true);
    assert_eq!(text, "data:text/plain;rule=esip6,hello");
    assert_eq!(
        parse_data_uri(&text).unwrap(),
        (b"hello".to_vec(), "text/plain;rule=esip6".to_string())
    );

    let binary = uri(&PNG_MAGIC, false, true);
    let (bytes, content_type) = parse_data_uri(&binary).unwrap();
    assert_eq!(bytes, PNG_MAGIC);
    assert_eq!(content_type, "image/png;rule=esip6");
}

#[test]
fn parse_defaults_the_content_type_when_build_skips_it() {
    assert_eq!(uri(b"hello", true, false), "data:,hello");
    assert_eq!(
        parse_data_uri("data:,hello").unwrap(),
        (b"hello".to_vec(), "text/plain;charset=US-ASCII".to_string())
    );

    let (bytes, content_type) = parse_data_uri(&uri(&PNG_MAGIC, true, false)).unwrap();
    assert_eq!(bytes, PNG_MAGIC);
    assert_eq!(content_type, "text/plain;charset=US-ASCII");
}

#[test]
fn parse_rejects_a_missing_prefix_or_comma() {
    assert_eq!(
        error_text("http://example.com"),
        "Internal error: Invalid data URI"
    );
    assert_eq!(
        error_text("data:text/plain"),
        "Internal error: Invalid data URI: missing comma"
    );
}

#[test]
fn parse_rejects_bad_payload_encoding() {
    assert!(
        error_text("data:text/plain;base64,!!!invalid!!!")
            .starts_with("Internal error: Failed to decode base64 data URI: ")
    );
    assert_eq!(
        error_text("data:,abc%4"),
        "Internal error: Invalid percent-encoding in data URI payload"
    );
    assert_eq!(
        error_text("data:,%zz"),
        "Internal error: Invalid percent-encoding in data URI payload"
    );
}

#[test]
fn prepare_builds_and_hashes_the_data_uri() {
    let prepared = prepare_payload(b"hello", &PrepareInscribeOptions::default()).unwrap();
    assert_eq!(prepared.content_type, "text/plain");
    assert_eq!(prepared.data_uri, "data:text/plain,hello");
    assert_eq!(prepared.data_uri_truncated, "data:text/plain,hello");
    assert_eq!(prepared.sha, compute_digest(b"data:text/plain,hello"));
    assert_eq!(prepared.size, 5);
    assert_eq!(prepared.gzipped_size, None);
}

#[test]
fn prepare_ignores_gzip_for_literal_input() {
    let options = PrepareInscribeOptions {
        gzip: true,
        ..PrepareInscribeOptions::default()
    };
    let prepared = prepare_payload(b"hello", &options).unwrap();
    assert_eq!(prepared.data_uri, "data:text/plain,hello");
    assert_eq!(prepared.gzipped_size, None);
    assert_eq!(prepared.size, 5);
}

#[test]
fn prepare_gzips_file_and_stdin_input() {
    let input = b"hello hello hello hello hello";
    let gzipped = gzip_bytes(input).unwrap();
    let expected = build_data_uri_with_type(&gzipped, "text/plain", &DataUriOptions::default());

    for source in [InputSource::File, InputSource::Stdin] {
        let options = PrepareInscribeOptions {
            source,
            gzip: true,
            ..PrepareInscribeOptions::default()
        };
        let prepared = prepare_payload(input, &options).unwrap();
        assert_eq!(prepared.content_type, "text/plain");
        assert_eq!(prepared.data_uri, expected);
        assert_eq!(prepared.sha, compute_digest(expected.as_bytes()));
        assert_eq!(prepared.gzipped_size, Some(gzipped.len()));
        assert_eq!(prepared.size, input.len());
    }
}

#[test]
fn prepare_applies_esip6_and_skip_content_type() {
    let options = PrepareInscribeOptions {
        esip6: true,
        skip_content_type: true,
        ..PrepareInscribeOptions::default()
    };
    let prepared = prepare_payload(b"hello", &options).unwrap();
    assert_eq!(prepared.content_type, "text/plain");
    assert_eq!(prepared.data_uri, "data:;rule=esip6,hello");
    assert_eq!(prepared.sha, compute_digest(b"data:;rule=esip6,hello"));
}

#[test]
fn prepare_truncates_only_the_preview() {
    let options = PrepareInscribeOptions {
        data_uri_limit: 10,
        ..PrepareInscribeOptions::default()
    };
    let prepared = prepare_payload(b"hello world", &options).unwrap();
    assert_eq!(prepared.data_uri, "data:text/plain,hello%20world");
    assert_eq!(prepared.data_uri_truncated, "data:text/...");

    let exact = PrepareInscribeOptions {
        data_uri_limit: 21,
        ..PrepareInscribeOptions::default()
    };
    let prepared = prepare_payload(b"hello", &exact).unwrap();
    assert_eq!(prepared.data_uri_truncated, "data:text/plain,hello");
}

#[test]
fn prepare_keeps_non_image_input_when_optimizing() {
    let json = br#"{"a":1}"#;
    let options = PrepareInscribeOptions {
        optimize: true,
        ..PrepareInscribeOptions::default()
    };
    let prepared = prepare_payload(json, &options).unwrap();
    assert_eq!(prepared.content_type, "application/json");
    assert_eq!(
        prepared.data_uri,
        build_data_uri(json, &DataUriOptions::default())
    );
    assert_eq!(prepared.size, json.len());
}

#[test]
fn prepare_converts_png_to_webp_when_optimizing() {
    let png = png();
    let webp = optimize_image(&png, "image/png").unwrap().unwrap();
    let expected = build_data_uri_with_type(&webp, "image/webp", &DataUriOptions::default());
    let options = PrepareInscribeOptions {
        optimize: true,
        ..PrepareInscribeOptions::default()
    };
    let prepared = prepare_payload(&png, &options).unwrap();
    assert_eq!(prepared.content_type, "image/webp");
    assert_eq!(prepared.data_uri, expected);
    assert_eq!(prepared.sha, compute_digest(expected.as_bytes()));
    assert_eq!(prepared.size, png.len());
    assert_eq!(prepared.gzipped_size, None);
}
