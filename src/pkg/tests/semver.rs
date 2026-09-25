//! Tag parsing, semver 2.0 precedence and the latest release.

use crate::pkg::semver::{latest_release, Version};
use crate::pkg::tests::support::v;

#[test]
fn parses_version_tags_and_ignores_others() {
    let x = Version::parse_tag("v1.2.3").expect("a release");
    assert_eq!((x.major, x.minor, x.patch, x.pre), (1, 2, 3, None));
    let p = Version::parse_tag("v1.0.0-rc.1").expect("a prerelease");
    assert_eq!(p.pre.as_deref(), Some("rc.1"));
    assert_eq!(p.to_string(), "v1.0.0-rc.1");
    for bad in [
        "1.2.3",
        "v1.2",
        "v1.2.3.4",
        "v01.2.3",
        "v1.2.3-",
        "v1.2.3-a..b",
        "v1.2.3-01",
        "v1.2.3+meta",
        "vx.y.z",
        "latest",
        "",
        "v1.2.3-a_b",
    ] {
        assert_eq!(Version::parse_tag(bad), None, "{bad:?}");
    }
}

#[test]
fn semver_precedence() {
    let order = [
        "v1.0.0-alpha",
        "v1.0.0-alpha.1",
        "v1.0.0-alpha.beta",
        "v1.0.0-beta",
        "v1.0.0-beta.2",
        "v1.0.0-beta.11",
        "v1.0.0-rc.1",
        "v1.0.0",
        "v1.0.1",
        "v1.2.0",
        "v2.0.0",
        "v10.0.0",
    ];
    for w in order.windows(2) {
        assert!(v(w[0]) < v(w[1]), "{} < {}", w[0], w[1]);
    }
}

#[test]
fn latest_release_skips_prereleases() {
    let vs = [v("v1.0.0"), v("v1.4.0"), v("v2.0.0-rc.1"), v("v1.3.9")];
    assert_eq!(latest_release(&vs), Some(&v("v1.4.0")));
    assert_eq!(latest_release(&[v("v1.0.0-rc.1")]), None);
}
