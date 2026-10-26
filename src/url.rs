pub fn query(url: &str) -> Vec<(String, String)> {
    url.split_once('?')
        .map_or("", |(_, q)| q)
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .collect()
}

pub fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(b) => {
                        out.push(b);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_decodes_repeated_keys_and_escapes() {
        let q = query("/api/log?ref=feature%2Flogin&ref=origin%2Fmain&repo=%2Fhome%2Fa+b&flag");
        assert_eq!(
            q,
            [("ref", "feature/login"), ("ref", "origin/main"), ("repo", "/home/a b"), ("flag", "")]
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
        );
    }

    #[test]
    fn decode_keeps_malformed_escapes() {
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%zz"), "%zz");
        assert_eq!(decode("%C3%A9"), "é");
    }

    #[test]
    fn encode_round_trips_through_decode() {
        let path = "/home/user/my repo/é&x=1";
        assert_eq!(decode(&encode(path)), path);
    }
}
