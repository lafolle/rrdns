extern crate itertools;
use itertools::Itertools;

pub fn zonify(domain: &str) -> String {
    format!(
        ".{}",
        domain.split('.').rev().intersperse(".").collect::<String>()
    )
}

// Given ".com.google.web" return ".com.google".
// Given "web.google.com." return "google.com.".
pub fn parent_zone(zone: &str) -> String {
    // It is not possible for zone to not have dot.
    let first_dot_index = match zone.find('.') {
        Some(x) => x as usize,
        None => panic!("zone does not have a dot"),
    };
    if first_dot_index == zone.len() - 1 {
        return ".".to_string();
    }
    zone[first_dot_index + 1..].to_string()
}

#[cfg(test)]
mod tests {
    use super::{parent_zone, zonify};

    #[test]
    fn test_zone_zonify_fqdn() {
        // Arrange
        let domain = "www.google.com";
        let expected_zone = ".com.google.www";

        // Act
        let actual_zone = zonify(domain);

        // Assert
        assert_eq!(expected_zone, actual_zone);
    }

    #[test]
    // PQDN - Partially Qualified Domain Name
    fn test_zone_zonify_pqdn() {
        // Arrange
        let domain = "com";
        let expected_zone = ".com";

        // Act
        let actual_zone = zonify(domain);

        // Assert
        assert_eq!(expected_zone, actual_zone);
    }

    #[test]
    fn test_zone_parent_zone_fqdn() {
        // Arrange
        let zone = "www.google.com.";
        let expected_zone = "google.com.";

        // Act
        let actual_zone = parent_zone(zone);

        // Assert
        assert_eq!(expected_zone, actual_zone);
    }

    #[test]
    fn test_zone_parent_zone_fqdn1() {
        // Arrange
        let zone = "google.com.";
        let expected_zone = "com.";

        // Act
        let actual_zone = parent_zone(zone);

        // Assert
        assert_eq!(expected_zone, actual_zone);
    }

    #[test]
    fn test_zone_parent_zone_fqdn2() {
        // Arrange
        let zone = "com.";
        let expected_zone = ".";

        // Act
        let actual_zone = parent_zone(zone);

        // Assert
        assert_eq!(expected_zone, actual_zone);
    }
}
