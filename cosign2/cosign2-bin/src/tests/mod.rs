use {
    self::input::Input,
    crate::ExitCode,
    std::io::{Read, Write},
};

mod input;

/// Try to dump header contents of a file with no header.
#[test]
fn dump_no_header() {
    let file = create_file(b"Hello, world!");
    let output = test(["dump", "-i", file.path().to_str().unwrap()]);
    assert_eq!(output.exit_code, ExitCode(0));
    assert!(output.stdout.contains("no header"));
    assert!(output.stderr.is_empty());
}

/// Dump header contents of a valid file with a header containing one signature.
#[test]
fn dump_valid_header_one_signature() {
    let (file, sig1, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: false,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let (sig1_half_1, sig1_half_2) = sig1.split_at(sig1.len() / 2);
    let output = test(["dump", "-i", file.path().to_str().unwrap()]);
    assert_eq!(output.exit_code, ExitCode(0));

    // Prints magic value.
    assert!(output.stdout.contains("atsama5d27-keyos"));
    // Prints human-readable timestamp in UTC.
    assert!(output.stdout.contains("2024-04-09 11:06:11 UTC"));
    // Prints timestamp as number of seconds since UNIX epoch.
    assert!(output.stdout.contains("1712660771"));
    // Prints human readable date as it is in the header.
    assert!(output.stdout.contains("Apr 09 2024"));
    // Prints version.
    assert!(output.stdout.contains("0.0.77"));
    // Prints firmware size.
    assert!(output.stdout.contains("13 B (13)"));
    // Prints first pubkey in hex.
    assert!(output.stdout.contains(input::PUBKEY_1_HEX));
    // Prints first signature in hex.
    assert!(output.stdout.contains(sig1_half_1));
    assert!(output.stdout.contains(sig1_half_2));
    // Prints second pubkey in hex (all zeros).
    assert!(output
        .stdout
        .contains("000000000000000000000000000000000000000000000000000000000000000000"));
    // Prints second signature in hex (all zeros).
    assert!(output
        .stdout
        .contains("0000000000000000000000000000000000000000000000000000000000000000"));

    assert!(output.stderr.is_empty());
}

/// Dump header contents of a valid file with a header containing two
/// signatures.
#[test]
fn dump_valid_header_two_signatures() {
    let (file, sig1, sig2) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: true,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let (sig1_half_1, sig1_half_2) = sig1.split_at(sig1.len() / 2);
    let (sig2_half_1, sig2_half_2) = sig2.split_at(sig2.len() / 2);
    let output = test(["dump", "-i", file.path().to_str().unwrap()]);
    assert_eq!(output.exit_code, ExitCode(0));

    // Prints magic value.
    assert!(output.stdout.contains("atsama5d27-keyos"));
    // Prints human-readable timestamp in UTC.
    assert!(output.stdout.contains("2024-04-09 11:06:11 UTC"));
    // Prints timestamp as number of seconds since UNIX epoch.
    assert!(output.stdout.contains("1712660771"));
    // Prints human readable date as it is in the header.
    assert!(output.stdout.contains("Apr 09 2024"));
    // Prints version.
    assert!(output.stdout.contains("0.0.77"));
    // Prints firmware size.
    assert!(output.stdout.contains("13 B (13)"));
    // Prints first pubkey in hex.
    assert!(output.stdout.contains(input::PUBKEY_1_HEX));
    // Prints first signature in hex.
    assert!(output.stdout.contains(sig1_half_1));
    assert!(output.stdout.contains(sig1_half_2));
    // Prints second pubkey in hex.
    assert!(output.stdout.contains(input::PUBKEY_2_HEX));
    // Prints second signature in hex.
    assert!(output.stdout.contains(sig2_half_1));
    assert!(output.stdout.contains(sig2_half_2));

    assert!(output.stderr.is_empty());
}

/// Dump header with the magic value changed. This is interpreted as a file with
/// no header, since the magic value is used to identify the header.
#[test]
fn dump_invalid_magic() {
    let (file, _, _) = input::Params {
        magic: false,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: true,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let output = test(["dump", "-i", file.path().to_str().unwrap()]);
    assert_eq!(output.exit_code, ExitCode(0));
    assert!(output.stdout.contains("no header"));
    assert!(output.stderr.is_empty());
}

/// Dump headers with various fields changed, hence invalidating the signatures.
#[test]
fn dump_invalid_changed_fields() {
    let cases = [
        |input: &mut Input| {
            input.timestamp[3] = 0xAA;
        },
        |input: &mut Input| {
            input.date[5] = 0xBB;
        },
        |input: &mut Input| {
            input.version[2] = 0xCC;
        },
        |input: &mut Input| {
            input.firmware_size[1] = 0xDD;
        },
        |input: &mut Input| {
            input.reserved[22] = 0xEE;
        },
        |input: &mut Input| {
            input.firmware[5] = 0xFF;
        },
    ];

    for (i, case) in cases.into_iter().enumerate() {
        let (file, _, _) = input::Params {
            magic: true,
            timestamp: 1712660771,
            date: b"Apr 09 2024".to_vec(),
            version: b"0.0.77".to_vec(),
            signature1: true,
            // Test some cases with two signatures.
            signature2: i % 2 == 0,
            reserved: Default::default(),
            firmware: b"Hello, world!".to_vec(),
            firmware_size: 13,
        }
        .input()
        .update(case)
        .create_file();
        let output = test(["dump", "-i", file.path().to_str().unwrap()]);
        assert_eq!(output.exit_code, ExitCode(1));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.contains("invalid signature1"));
    }
}

/// Dump header with the first pubkey changed.
#[test]
fn dump_invalid_header_invalid_pubkey1() {
    let (file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: true,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .update(|input| {
        input.pubkey1[5] = 0xFF;
    })
    .create_file();
    let output = test(["dump", "-i", file.path().to_str().unwrap()]);
    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.contains("invalid signature1"));
}

/// Dump header with the second pubkey changed.
#[test]
fn dump_invalid_header_invalid_pubkey2() {
    let (file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: true,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .update(|input| {
        input.pubkey2[5] = 0xFF;
    })
    .create_file();
    let output = test(["dump", "-i", file.path().to_str().unwrap()]);
    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.contains("invalid signature2"));
}

/// Dump header with the first signature changed.
#[test]
fn dump_invalid_header_invalid_signature1() {
    let (file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: true,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .update(|input| {
        input.signature1[5] = 0xDB;
    })
    .create_file();
    let output = test(["dump", "-i", file.path().to_str().unwrap()]);
    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.contains("invalid signature1"));
}

/// Dump header with the second signature changed.
#[test]
fn dump_invalid_header_invalid_signature2() {
    let (file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: true,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .update(|input| {
        input.signature2[5] = 0xDB;
    })
    .create_file();
    let output = test(["dump", "-i", file.path().to_str().unwrap()]);
    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.contains("invalid signature2"));
}

/// Sign an image with two trusted keys (i.e. not developer keys).
#[test]
fn sign_with_trusted_keys() {
    // Sign the image with the first key.
    let input_file = create_file(b"Hello, world!");
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_1_PEM);
    let output = test([
        "sign",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "-i",
        input_file.path().to_str().unwrap(),
        "--firmware-version",
        "1.2.4-alpha1",
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
    ]);

    assert_eq!(output.exit_code, ExitCode(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    let output = test(["dump", "-i", output_file.path().to_str().unwrap()]);

    assert_eq!(output.exit_code, ExitCode(0));
    assert!(output.stdout.contains("atsama5d27-keyos"));
    assert!(output.stdout.contains("1.2.4-alpha1"));
    assert!(output.stdout.contains("13 B (13)"));
    assert!(output.stdout.contains(input::PUBKEY_1_HEX));
    assert!(output
        .stdout
        .contains("000000000000000000000000000000000000000000000000000000000000000000"));
    assert!(output.stderr.is_empty());

    // Sign the image with the second key.
    let secret_pem = create_file(input::SECRET_2_PEM);
    let final_output_file = tempfile::NamedTempFile::new().unwrap();
    let output = test([
        "sign",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "-i",
        output_file.path().to_str().unwrap(),
        "--target",
        "atsama5d27-keyos",
        "-o",
        final_output_file.path().to_str().unwrap(),
    ]);

    assert_eq!(output.exit_code, ExitCode(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    let output = test(["dump", "-i", final_output_file.path().to_str().unwrap()]);

    assert_eq!(output.exit_code, ExitCode(0));
    assert!(output.stdout.contains("atsama5d27-keyos"));
    assert!(output.stdout.contains("1.2.4-alpha1"));
    assert!(output.stdout.contains("13 B (13)"));
    assert!(output.stdout.contains(input::PUBKEY_1_HEX));
    assert!(output.stdout.contains(input::PUBKEY_2_HEX));
    assert!(!output
        .stdout
        .contains("000000000000000000000000000000000000000000000000000000000000000000"));
    assert!(!output
        .stdout
        .contains("0000000000000000000000000000000000000000000000000000000000000000"));
    assert!(output.stderr.is_empty());
}

/// Sign an image with a trusted key loaded from config.
#[test]
fn sign_with_trusted_key_from_config() {
    let (input_file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: false,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_2_PEM);
    let config_file = create_file(
        format!(
            r#"
            pubkey = "{}"
            secret = "{}"
            known_pubkeys = ["{}", "{}"]
            target = "atsama5d27-keyos"
            "#,
            input::PUBKEY_2_HEX,
            secret_pem.path().to_str().unwrap(),
            input::PUBKEY_1_HEX,
            input::PUBKEY_2_HEX,
        )
        .as_bytes(),
    );

    let output = test([
        "sign",
        "-i",
        input_file.path().to_str().unwrap(),
        "--config",
        config_file.path().to_str().unwrap(),
        "-o",
        output_file.path().to_str().unwrap(),
    ]);

    assert_eq!(output.exit_code, ExitCode(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    let output = test(["dump", "-i", output_file.path().to_str().unwrap()]);

    assert_eq!(output.exit_code, ExitCode(0));
    assert!(output.stdout.contains("atsama5d27-keyos"));
    assert!(output.stdout.contains("0.0.77"));
    assert!(output.stdout.contains("13 B (13)"));
    assert!(output.stdout.contains(input::PUBKEY_1_HEX));
    assert!(output.stdout.contains(input::PUBKEY_2_HEX));
    assert!(!output
        .stdout
        .contains("000000000000000000000000000000000000000000000000000000000000000000"));
    assert!(!output
        .stdout
        .contains("0000000000000000000000000000000000000000000000000000000000000000"));
    assert!(output.stderr.is_empty());
}

/// Attempt to specify the pubkey both in the config file and on the CLI.
#[test]
fn pubkey_in_config_and_cli() {
    let (input_file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: false,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_2_PEM);
    let config_file = create_file(
        format!(
            r#"
            pubkey = "{}"
            "#,
            input::PUBKEY_2_HEX,
        )
        .as_bytes(),
    );

    let output = test([
        "sign",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "--pubkey",
        input::PUBKEY_1_HEX,
        "-i",
        input_file.path().to_str().unwrap(),
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
        "--config",
        config_file.path().to_str().unwrap(),
    ]);

    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output
        .stderr
        .contains("pubkey specified in both config and cli"));
    let mut buf = Vec::new();
    output_file.as_file().read_to_end(&mut buf).unwrap();
    assert!(buf.is_empty());
}

/// Attempt to specify the secret both in the config file and on the CLI.
#[test]
fn secret_in_config_and_cli() {
    let (input_file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: false,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_2_PEM);
    let config_file = create_file(
        format!(
            r#"
            secret = "{}"
            "#,
            secret_pem.path().to_str().unwrap(),
        )
        .as_bytes(),
    );

    let output = test([
        "sign",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "--pubkey",
        input::PUBKEY_1_HEX,
        "-i",
        input_file.path().to_str().unwrap(),
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
        "--config",
        config_file.path().to_str().unwrap(),
    ]);

    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output
        .stderr
        .contains("secret specified in both config and cli"));
    let mut buf = Vec::new();
    output_file.as_file().read_to_end(&mut buf).unwrap();
    assert!(buf.is_empty());
}

/// Attempt to specify the known pubkeys both in the config file and on the CLI.
#[test]
fn known_pubkeys_in_config_and_cli() {
    let (input_file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: false,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_2_PEM);
    let config_file = create_file(
        format!(
            r#"
            known_pubkeys = ["{}", "{}"]
            "#,
            input::PUBKEY_2_HEX,
            input::PUBKEY_1_HEX,
        )
        .as_bytes(),
    );

    let output = test([
        "sign",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "--pubkey",
        input::PUBKEY_1_HEX,
        "-i",
        input_file.path().to_str().unwrap(),
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
        "--config",
        config_file.path().to_str().unwrap(),
        "--known-pubkey",
        input::PUBKEY_2_HEX,
    ]);

    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output
        .stderr
        .contains("known pubkeys specified in both config and cli"));
    let mut buf = Vec::new();
    output_file.as_file().read_to_end(&mut buf).unwrap();
    assert!(buf.is_empty());
}

/// Attempt to specify the target both in the config file and on the CLI.
#[test]
fn target_in_config_and_cli() {
    let (input_file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: false,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_2_PEM);
    let config_file = create_file(
        r#"
        target = "atsama5d27-keyos"
        "#
        .as_bytes(),
    );

    let output = test([
        "sign",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "--pubkey",
        input::PUBKEY_1_HEX,
        "-i",
        input_file.path().to_str().unwrap(),
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
        "--config",
        config_file.path().to_str().unwrap(),
        "--known-pubkey",
        input::PUBKEY_2_HEX,
    ]);

    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output
        .stderr
        .contains("target specified in both config and cli"));
    let mut buf = Vec::new();
    output_file.as_file().read_to_end(&mut buf).unwrap();
    assert!(buf.is_empty());
}

/// Sign an image with a single developer key.
#[test]
fn sign_with_developer_key() {
    let input_file = create_file(b"Hello, world!");
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_1_PEM);
    let output = test([
        "sign",
        "--developer",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "-i",
        input_file.path().to_str().unwrap(),
        "--firmware-version",
        "1.2.4-alpha1",
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
    ]);

    assert_eq!(output.exit_code, ExitCode(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    let output = test(["dump", "-i", output_file.path().to_str().unwrap()]);

    assert_eq!(output.exit_code, ExitCode(0));
    assert!(output.stdout.contains("atsama5d27-keyos"));
    assert!(output.stdout.contains("1.2.4-alpha1"));
    assert!(output.stdout.contains("13 B (13)"));
    assert!(output.stdout.contains(input::PUBKEY_1_HEX));
    assert!(output
        .stdout
        .contains("000000000000000000000000000000000000000000000000000000000000000000"));
    assert!(output.stderr.is_empty());
}

/// Sign an image in place with a single developer key.
#[test]
fn sign_with_developer_key_in_place() {
    let input_file = create_file(b"Hello, World!");
    let secret_pem = create_file(input::SECRET_1_PEM);
    let output = test([
        "sign",
        "--developer",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "-i",
        input_file.path().to_str().unwrap(),
        "--firmware-version",
        "1.2.4-alpha1",
        "--target",
        "atsama5d27-keyos",
        "--in-place",
    ]);

    assert_eq!(output.exit_code, ExitCode(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    let output = test(["dump", "-i", input_file.path().to_str().unwrap()]);

    assert_eq!(output.exit_code, ExitCode(0));
    assert!(output.stdout.contains("atsama5d27-keyos"));
    assert!(output.stdout.contains("1.2.4-alpha1"));
    assert!(output.stdout.contains("13 B (13)"));
    assert!(output.stdout.contains(input::PUBKEY_1_HEX));
    assert!(output
        .stdout
        .contains("000000000000000000000000000000000000000000000000000000000000000000"));
    assert!(output.stderr.is_empty());
}

/// Attempt to sign an image in place while also specifying an output file.
#[test]
fn sign_with_developer_key_in_place_with_output_file() {
    let input_file = create_file(b"Hello, World!");
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_1_PEM);
    let output = test([
        "sign",
        "--developer",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "-i",
        input_file.path().to_str().unwrap(),
        "--firmware-version",
        "1.2.4-alpha1",
        "--target",
        "atsama5d27-keyos",
        "--in-place",
        "-o",
        output_file.path().to_str().unwrap(),
    ]);

    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output
        .stderr
        .contains("cannot specify both --in-place and --output (-o)"));
    let mut buf = Vec::new();
    output_file.as_file().read_to_end(&mut buf).unwrap();
    assert!(buf.is_empty());
}

/// Attempt to resign an image which has already been signed with a developer
/// key.
#[test]
fn resign_with_developer_key() {
    let (input_file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: false,
        signature2: true,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_1_PEM);
    let output = test([
        "sign",
        "--developer",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "-i",
        input_file.path().to_str().unwrap(),
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
    ]);

    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.contains("signature2 already present"));
    let mut buf = Vec::new();
    output_file.as_file().read_to_end(&mut buf).unwrap();
    assert!(buf.is_empty());
}

/// Attempt to sign with a private key that doesn't match the specified public
/// key.
#[test]
fn sign_pubkey_mismatch() {
    let input_file = create_file(b"Hello, World!");
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_2_PEM);
    let output = test([
        "sign",
        "--developer",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "--pubkey",
        input::PUBKEY_1_HEX,
        "-i",
        input_file.path().to_str().unwrap(),
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
    ]);

    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output
        .stderr
        .contains("public key does not match secret key"));
    let mut buf = Vec::new();
    output_file.as_file().read_to_end(&mut buf).unwrap();
    assert!(buf.is_empty());
}

/// Attempt to resign an image which has already been signed with the given
/// trusted key.
#[test]
fn resign_with_trusted_key() {
    let (input_file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: false,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_1_PEM);
    let output = test([
        "sign",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "-i",
        input_file.path().to_str().unwrap(),
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
    ]);

    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.contains("same pubkey twice"));
    let mut buf = Vec::new();
    output_file.as_file().read_to_end(&mut buf).unwrap();
    assert!(buf.is_empty());
}

/// Attempt to sign an image which has already been signed by an unknown key.
#[test]
fn sign_header_contains_unknown_pubkey() {
    let (input_file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: false,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_2_PEM);
    let output = test([
        "sign",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "-i",
        input_file.path().to_str().unwrap(),
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
        "--known-pubkey",
        input::PUBKEY_2_HEX,
    ]);

    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.contains("unknown pubkey1"));
    let mut buf = Vec::new();
    output_file.as_file().read_to_end(&mut buf).unwrap();
    assert!(buf.is_empty());
}

/// Attempt to sign an image using an unknown key.
#[test]
fn sign_header_with_unknown_pubkey() {
    let (input_file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: false,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_2_PEM);
    let output = test([
        "sign",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "-i",
        input_file.path().to_str().unwrap(),
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
        "--known-pubkey",
        input::PUBKEY_1_HEX,
    ]);

    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output
        .stderr
        .contains("signing key is not in known pubkeys list"));
    let mut buf = Vec::new();
    output_file.as_file().read_to_end(&mut buf).unwrap();
    assert!(buf.is_empty());
}

/// Sign an image with a second key which has already been signed by a
/// known key.
#[test]
fn sign_header_with_known_pubkeys() {
    let (input_file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.77".to_vec(),
        signature1: true,
        signature2: false,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_2_PEM);
    let output = test([
        "sign",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "-i",
        input_file.path().to_str().unwrap(),
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
        "--known-pubkey",
        input::PUBKEY_2_HEX,
        "--known-pubkey",
        input::PUBKEY_1_HEX,
    ]);

    assert_eq!(output.exit_code, ExitCode(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

/// Attempt to sign an image with an invalid version UTF-8 in the header.
#[test]
fn sign_header_invalid_version_utf8() {
    let (input_file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: vec![0xFF; 5],
        signature1: true,
        signature2: false,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_2_PEM);
    let output = test([
        "sign",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "-i",
        input_file.path().to_str().unwrap(),
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
    ]);

    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.contains("invalid version UTF-8"));
}

/// Attempt to sign an image with an invalid version SemVer in the header.
#[test]
fn sign_header_invalid_version_semver() {
    let (input_file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"invalid".to_vec(),
        signature1: true,
        signature2: false,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_2_PEM);
    let output = test([
        "sign",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "-i",
        input_file.path().to_str().unwrap(),
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
    ]);

    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.contains("invalid version SemVer"));
}

/// Attempt to sign an image with an invalid date UTF-8 in the header.
#[test]
fn sign_header_invalid_date_utf8() {
    let (input_file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: vec![0xFF; 5],
        version: b"0.0.7".to_vec(),
        signature1: true,
        signature2: false,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 13,
    }
    .input()
    .create_file();
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_2_PEM);
    let output = test([
        "sign",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "-i",
        input_file.path().to_str().unwrap(),
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
    ]);

    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.contains("invalid date UTF-8"));
}

/// Attempt to sign an image with an invalid firmware size.
#[test]
fn sign_header_invalid_firmware_size() {
    let (input_file, _, _) = input::Params {
        magic: true,
        timestamp: 1712660771,
        date: b"Apr 09 2024".to_vec(),
        version: b"0.0.7".to_vec(),
        signature1: true,
        signature2: false,
        reserved: Default::default(),
        firmware: b"Hello, world!".to_vec(),
        firmware_size: 53,
    }
    .input()
    .create_file();
    let output_file = tempfile::NamedTempFile::new().unwrap();
    let secret_pem = create_file(input::SECRET_1_PEM);
    let output = test([
        "sign",
        "--secret",
        secret_pem.path().to_str().unwrap(),
        "-i",
        input_file.path().to_str().unwrap(),
        "--target",
        "atsama5d27-keyos",
        "-o",
        output_file.path().to_str().unwrap(),
    ]);

    assert_eq!(output.exit_code, ExitCode(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.contains("invalid firmware size"));
    assert!(output.stderr.contains("53"));
    assert!(output.stderr.contains("13"));
}

fn test<const N: usize>(args: [&str; N]) -> Output {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit_code = crate::main_args(
        std::iter::once("cosign2").chain(args),
        &mut stdout,
        &mut stderr,
    );
    println!("* args: {:?}", args);
    println!("* exit_code: {:?}", exit_code);
    println!("* stdout:\n{}", String::from_utf8_lossy(&stdout));
    println!("* stderr:\n{}", String::from_utf8_lossy(&stderr));
    Output {
        exit_code,
        stdout: String::from_utf8(stdout).unwrap(),
        stderr: String::from_utf8(stderr).unwrap(),
    }
}

#[derive(Debug)]
struct Output {
    exit_code: ExitCode,
    stdout: String,
    stderr: String,
}

fn create_file(data: &[u8]) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(data).unwrap();
    file
}
