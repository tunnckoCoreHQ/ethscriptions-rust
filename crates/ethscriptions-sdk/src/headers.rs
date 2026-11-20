// SPDX-License-Identifier: MPL-2.0

use http::HeaderMap;

/// Build CDN cache headers targeting Cloudflare, Netlify, and Vercel simultaneously.
///
/// Returns a `HeaderMap` with:
/// - `Cache-Control: public, max-age={ttl}, s-maxage={ttl}`
/// - `CDN-Cache-Control: public, s-maxage={ttl}, must-revalidate`
/// - `Cloudflare-CDN-Cache-Control: public, s-maxage={ttl}, must-revalidate`
/// - `Netlify-CDN-Cache-Control: public, s-maxage={ttl}, must-revalidate, durable`
/// - `Vercel-CDN-Cache-Control: public, s-maxage={ttl}, must-revalidate`
/// - `Vary: Accept, Accept-Encoding`
pub fn cache_headers(ttl: u64) -> HeaderMap {
    let mut headers = HeaderMap::new();

    let cache_control = format!("public, max-age={ttl}, s-maxage={ttl}");
    let cdn_control = format!("public, s-maxage={ttl}, must-revalidate");
    let netlify_cdn = format!("public, s-maxage={ttl}, must-revalidate, durable");
    let cf_cdn = format!("public, s-maxage={ttl}, must-revalidate");

    headers.insert("cache-control", cache_control.parse().unwrap());
    headers.insert("cdn-cache-control", cdn_control.parse().unwrap());
    headers.insert("cloudflare-cdn-cache-control", cf_cdn.parse().unwrap());
    headers.insert("netlify-cdn-cache-control", netlify_cdn.parse().unwrap());
    headers.insert("vercel-cdn-cache-control", cdn_control.parse().unwrap());
    headers.insert("vary", "Accept, Accept-Encoding".parse().unwrap());

    headers
}

/// Build cache headers with an additional Content-Type header.
pub fn cache_headers_with_content_type(ttl: u64, content_type: &str) -> HeaderMap {
    let mut headers = cache_headers(ttl);
    headers.insert("content-type", content_type.parse().unwrap());
    headers
}

// ---------------------------------------------------------------------------
// Per-endpoint TTL constants (in seconds)
// ---------------------------------------------------------------------------

/// TTL for immutable content: single ethscription metadata, content, attachment, check_exists, resolve.
pub const TTL_IMMUTABLE: u64 = 3600;

/// TTL for user profiles.
pub const TTL_PROFILE: u64 = 300;

/// TTL for numbers/stats endpoints.
pub const TTL_NUMBERS: u64 = 60;

/// TTL for transfers and owners.
pub const TTL_TRANSFERS: u64 = 45;

/// TTL for list endpoints.
pub const TTL_LIST: u64 = 15;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_headers_has_all_providers() {
        let headers = cache_headers(300);

        assert_eq!(headers.contains_key("cache-control"), true);
        assert_eq!(headers.contains_key("cdn-cache-control"), true);
        assert_eq!(headers.contains_key("cloudflare-cdn-cache-control"), true);
        assert_eq!(headers.contains_key("netlify-cdn-cache-control"), true);
        assert_eq!(headers.contains_key("vercel-cdn-cache-control"), true);
        assert_eq!(headers.contains_key("vary"), true);
    }

    #[test]
    fn test_cache_headers_ttl_value() {
        let headers = cache_headers(60);
        let cc = headers.get("cache-control").unwrap().to_str().unwrap();
        assert_eq!(cc.contains("max-age=60"), true);
        assert_eq!(cc.contains("s-maxage=60"), true);
    }

    #[test]
    fn test_cache_headers_netlify_durable() {
        let headers = cache_headers(45);
        let netlify = headers
            .get("netlify-cdn-cache-control")
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(netlify.contains("durable"), true);
    }

    #[test]
    fn test_cache_headers_with_content_type() {
        let headers = cache_headers_with_content_type(3600, "image/png");
        assert_eq!(
            headers.get("content-type").unwrap().to_str().unwrap(),
            "image/png"
        );
        assert_eq!(headers.contains_key("cache-control"), true);
    }

    #[test]
    fn test_vary_header() {
        let headers = cache_headers(100);
        let vary = headers.get("vary").unwrap().to_str().unwrap();
        assert_eq!(vary, "Accept, Accept-Encoding");
    }
}
