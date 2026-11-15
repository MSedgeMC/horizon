//! OldUsersConverter: the pre-1.7 text user lists (`banned-ips.txt`,
//! `banned-players.txt`, `ops.txt`, `white-list.txt`) a server converts when it
//! first starts on a world from those versions. Only the IP ban list is
//! converted here: it needs no profile lookup, unlike the lists of names.
use super::user_lists::{BanEntry, IpBanList, parse_date};
use std::path::Path;

/// `OldUsersConverter.OLD_IPBANLIST`.
pub const OLD_IP_BAN_LIST: &str = "banned-ips.txt";

/// `OldUsersConverter.readOldListFormat`: the lines that are not a comment or
/// blank, each split on `|` (Java's split drops the trailing empty fields), the
/// first field lowercased as the map key (Locale.ROOT).
pub fn read_old_list_format(text: &str) -> Vec<(String, Vec<String>)> {
    let mut users = Vec::new();
    for line in text.split(['\n', '\r']) {
        // Java's String.trim: only characters up to U+0020.
        let line = line.trim_matches(|c: char| c <= ' ');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts: Vec<String> = line.split('|').map(str::to_owned).collect();
        while parts.len() > 1 && parts.last().is_some_and(String::is_empty) {
            parts.pop();
        }
        let key = parts[0].to_lowercase();
        users.push((key, parts));
    }
    users
}

/// `OldUsersConverter.convertIpBanlist` for the working directory, as vanilla's
/// `File("banned-ips.txt")` reads it.
pub fn convert_ip_banlist() -> bool {
    convert_ip_banlist_in(Path::new("."))
}

/// Convert `dir/banned-ips.txt` into `dir/banned-ips.json`, keeping the entries
/// the json file already has, then rename the old file to `.converted`
/// (`OldUsersConverter.convertIpBanlist`). True when there is nothing to
/// convert or it converted; false on a read or write failure, which vanilla
/// retries.
pub fn convert_ip_banlist_in(dir: &Path) -> bool {
    let old = dir.join(OLD_IP_BAN_LIST);
    if !old.is_file() {
        return true;
    }
    let mut bans = IpBanList::new(dir.join("banned-ips.json"));
    if bans.file.exists() {
        if let Err(e) = bans.load() {
            log::warn!("Could not load existing file banned-ips.json: {e}");
        }
    }
    let text = match super::user_lists::read_utf8(&old) {
        Ok(text) => text,
        Err(e) => {
            log::warn!("Could not parse old ip banlist to convert it! {e}");
            return false;
        }
    };
    for (ip, parts) in read_old_list_format(&text) {
        let created = parts.get(1).and_then(|s| parse_date(s));
        let source = parts.get(2).cloned();
        let expires = parts.get(3).and_then(|s| parse_date(s));
        let reason = parts.get(4).cloned();
        bans.add(BanEntry::with_created(ip, created, source, expires, reason));
    }
    if let Err(e) = bans.save() {
        log::warn!("Could not parse old ip banlist to convert it! {e}");
        return false;
    }
    // Vanilla ignores a rename that fails; the write above is the conversion.
    let _ = std::fs::rename(&old, dir.join(format!("{OLD_IP_BAN_LIST}.converted")));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The old line format: comments and blanks skipped, fields split on `|`
    /// with trailing empties dropped, the key lowercased and the whole line
    /// trimmed of characters up to U+0020 only; then a conversion merges into
    /// the json list and renames the old file.
    #[test]
    fn converts_an_old_ip_ban_list() {
        let users = read_old_list_format(
            "# comment\r\n\r\n 10.0.0.1|2020-02-03 04:05:06 +0000|Server|2021-01-01 00:00:00 +0000|grief|  \n10.0.0.2|",
        );
        assert_eq!(
            users,
            [
                (
                    "10.0.0.1".to_owned(),
                    [
                        "10.0.0.1",
                        "2020-02-03 04:05:06 +0000",
                        "Server",
                        "2021-01-01 00:00:00 +0000",
                        "grief"
                    ]
                    .map(str::to_owned)
                    .to_vec()
                ),
                ("10.0.0.2".to_owned(), vec!["10.0.0.2".to_owned()]),
            ]
        );
        let dir = std::env::temp_dir().join(format!("horizon-old-ips-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("banned-ips.json"),
            "[{\"ip\":\"9.9.9.9\",\"created\":\"2020-01-01 00:00:00 +0000\",\"source\":\"Server\",\"expires\":\"forever\",\"reason\":null}]",
        )
        .unwrap();
        std::fs::write(
            dir.join(OLD_IP_BAN_LIST),
            "# old bans\n10.0.0.1|2020-02-03 04:05:06 +0000|Server|2031-01-01 00:00:00 +0000|grief\n10.0.0.2\n",
        )
        .unwrap();
        assert!(convert_ip_banlist_in(&dir));
        assert!(!dir.join(OLD_IP_BAN_LIST).exists());
        assert!(dir.join("banned-ips.txt.converted").exists());
        let mut after = IpBanList::new(dir.join("banned-ips.json"));
        after.load().unwrap();
        let found = |ip: &str| {
            after
                .entries()
                .into_iter()
                .find(|e| e.user == ip)
                .cloned()
                .unwrap()
        };
        assert_eq!(found("9.9.9.9").reason, None);
        let converted = found("10.0.0.1");
        assert_eq!(
            converted.created,
            parse_date("2020-02-03 04:05:06 +0000").unwrap()
        );
        assert_eq!(converted.source, "Server");
        assert_eq!(converted.expires, parse_date("2031-01-01 00:00:00 +0000"));
        assert_eq!(converted.reason.as_deref(), Some("grief"));
        // A line with just the address gets the constructor's defaults.
        let bare = found("10.0.0.2");
        assert_eq!(bare.source, "(Unknown)");
        assert_eq!(bare.expires, None);
        assert_eq!(bare.reason, None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
