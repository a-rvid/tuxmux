fn main() {
    let domain = std::env::var("C2_DOMAIN").expect("C2_DOMAIN env var required");

    let ip = std::env::var("DNS_SERVER").expect("DNS_SERVER env var required");

    let octets: Vec<u8> = ip.split('.').map(|s| s.parse().unwrap()).collect();
    assert!(
        octets.len() == 4,
        "DNS_SERVER must be an IPv4 address with 4 octets"
    );

    println!("cargo:rustc-env=C2_DOMAIN={}", domain);
    println!("cargo:rustc-env=DNS_1={}", octets[0]);
    println!("cargo:rustc-env=DNS_2={}", octets[1]);
    println!("cargo:rustc-env=DNS_3={}", octets[2]);
    println!("cargo:rustc-env=DNS_4={}", octets[3]);
}
