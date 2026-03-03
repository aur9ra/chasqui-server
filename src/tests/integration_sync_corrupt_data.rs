use crate::features::model::{Feature, FeatureType};
use crate::services::sync::SyncService;
use crate::tests::integration_sync_core::mock_config;
use crate::tests::mocks::{MockBuildNotifier, MockContentReader, MockRepository};
use std::path::PathBuf;
use std::sync::Arc;

/// A collection of strings designed to break parsers, databases, and file systems.
const MALICIOUS_PAYLOADS: &[&str] = &[
    "../../etc/passwd",
    "C:\\Windows\\System32\\drivers\\etc\\hosts",
    "<script>alert('xss')</script>",
    "'; DROP TABLE pages; --",
    "\" OR 1=1 --",
    "\0null\0byte\0",
    "\\u{202e}RTL_OVERRIDE", // Right-to-left override (escaped to avoid compiler error)
    "; ls -la",
    "$(rm -rf /)",
    "   ", // Whitespace only
    "\\n\\r\\t",
    "❤️🔥🚀", // Multibyte Emojis
];

/// Generates a sequence of non-UTF8 garbage bytes.
fn get_garbage_bytes(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 255) as u8).collect()
}

#[tokio::test]
async fn test_sync_corrupt_page_handles_gracefully() {
    let repo = MockRepository::new();
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let content_dir = PathBuf::from("/content");
    let config = mock_config(content_dir.clone());

    // --- The Combinatorial Foundry ---
    // We will test 6 categories of corruption across 64 combinations (2^6)
    // For each combination, we create 5-10 files.

    let mut file_count = 0;

    for i in 0..64 {
        let corrupt_filename = (i >> 0) & 1 == 1;
        let corrupt_id = (i >> 1) & 1 == 1;
        let corrupt_name = (i >> 2) & 1 == 1;
        let corrupt_tags = (i >> 3) & 1 == 1;
        let corrupt_date = (i >> 4) & 1 == 1;
        let corrupt_body = (i >> 5) & 1 == 1;

        for sub_id in 0..5 {
            let payload_idx = (i + sub_id) % MALICIOUS_PAYLOADS.len();
            let payload = MALICIOUS_PAYLOADS[payload_idx];

            // 1. Construct Filename
            let mut filename = format!("page_{}_{}.md", i, sub_id);
            if corrupt_filename {
                // Warning: Real filesystems might reject some of these, but our Mock handles them.
                filename = format!(
                    "corrupt_{}_{}_{}",
                    i,
                    sub_id,
                    payload.replace("/", "_").replace("\\", "_")
                );
                if !filename.ends_with(".md") {
                    filename.push_str(".md");
                }
            }

            // 2. Construct Frontmatter
            let mut fm = String::from("---\n");
            if corrupt_id {
                fm.push_str(&format!("identifier: \"{}\"\n", payload));
            }
            if corrupt_name {
                fm.push_str(&format!("name: \"{}\"\n", payload));
            }
            if corrupt_tags {
                fm.push_str("tags:\n");
                fm.push_str(&format!("  - \"{}\"\n", payload));
                fm.push_str("  - \"normal-tag\"\n");
            }
            if corrupt_date {
                fm.push_str(&format!(
                    "modified_datetime: \"{} garbage-date\"\n",
                    payload
                ));
            }
            fm.push_str("---\n");

            // 3. Construct Body
            let mut body = String::from("# Content\n");
            if corrupt_body {
                // Mix in massive strings and binary garbage
                body.push_str(&"A".repeat(10000));
                body.push_str(payload);
            }

            let full_path = format!("/content/md/{}", filename);

            // Randomly decide between string or binary write to test UTF-8 resilience
            if corrupt_body && sub_id == 0 {
                let mut bytes = fm.as_bytes().to_vec();
                bytes.extend_from_slice(&get_garbage_bytes(1024 * 10)); // 1KB of noise
                reader.add_binary_file(&full_path, bytes);
            } else {
                reader.add_file(&full_path, &format!("{}{}", fm, body));
            }

            file_count += 1;
        }
    }

    println!(
        "Foundry: Generated {} test files across 64 corruption profiles.",
        file_count
    );

    let service = SyncService::new(
        Box::new(repo.clone()),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    )
    .await
    .expect("Sync Service MUST NOT panic during initialization, even with garbage files");

    // Start-up sync already happened in ::new(), but we can trigger it again to check for idempotency/hangs
    service
        .full_sync()
        .await
        .expect("Sync Service MUST NOT fail globally due to individual file corruption");

    // --- Verification Pass ---
    let pages = service.get_all_features_by_type(FeatureType::Page).await;

    // We expect some files to be rejected (e.g. invalid UTF-8 in crucial places)
    // but the system must remain operational.
    assert!(
        pages.len() > 0,
        "System should have recovered at least some valid pages"
    );

    for page_feat in pages {
        if let Feature::Page(p) = page_feat {
            // Identifier check: Ensure no traversal escape
            assert!(
                !p.identifier.contains("../"),
                "Identifier sanitization failed for: {}",
                p.identifier
            );
            assert!(
                !p.identifier.contains("..\\"),
                "Identifier sanitization failed for: {}",
                p.identifier
            );

            // Database Integrity: Verify name/tags didn't break the SQLite repository (verified by the fact we retrieved them)
            if let Some(ref name) = p.name {
                assert!(name.len() <= 100000, "Unexpectedly large name string");
            }
        }
    }

    println!(
        "Foundry: All {} pages processed without system failure.",
        file_count
    );
}

#[tokio::test]
async fn test_sync_corrupt_media_handles_gracefully() {
    let repo = MockRepository::new();
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let content_dir = PathBuf::from("/content");
    let config = mock_config(content_dir.clone());

    let mut file_count = 0;

    // 1. Generate Corrupt Videos
    for i in 0..10 {
        let path = format!("/content/videos/corrupt_{}.mp4", i);
        let mut bytes = vec![0x00, 0x00, 0x00, 0x20, 0x66, 0x74, 0x79, 0x70]; // mp4 signature
        bytes.extend_from_slice(&get_garbage_bytes(1024 * i));
        reader.add_binary_file(&path, bytes);
        file_count += 1;
    }

    // 2. Generate Corrupt Audio
    for i in 0..10 {
        let path = format!("/content/audio/corrupt_{}.mp3", i);
        let mut bytes = vec![0x49, 0x44, 0x33]; // mp3 ID3 signature
        bytes.extend_from_slice(&get_garbage_bytes(512 * i));
        reader.add_binary_file(&path, bytes);
        file_count += 1;
    }

    // 3. Generate Corrupt Images
    for i in 0..10 {
        let path = format!("/content/images/corrupt_{}.png", i);
        let mut bytes = vec![0x89, 0x50, 0x4E, 0x47]; // png signature
        bytes.extend_from_slice(&get_garbage_bytes(256 * i));
        reader.add_binary_file(&path, bytes);
        file_count += 1;
    }

    // 4. Identity Theft / Mismatched extensions
    // MD file pretending to be MP4
    reader.add_file(
        "/content/videos/not_a_video.mp4",
        "# This is actually markdown",
    );
    // Binary file pretending to be MD
    reader.add_binary_file("/content/md/not_a_page.md", get_garbage_bytes(1000));
    file_count += 2;

    println!("Foundry: Generated {} corrupt media files.", file_count);

    let service = SyncService::new(
        Box::new(repo.clone()),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    )
    .await
    .expect("Sync service MUST survive corrupt media initialization");

    service
        .full_sync()
        .await
        .expect("Sync service MUST survive corrupt media batch");

    // Verification
    let manifest = service.manifest.read().await;

    // We expect the system to have registered these files regardless of their internal corruption,
    // because Chasqui prioritizes the physical existence of the source asset.
    assert!(manifest.filenames.contains("corrupt_5.mp4"));
    assert!(manifest.filenames.contains("corrupt_5.mp3"));
    assert!(manifest.filenames.contains("corrupt_5.png"));
    assert!(manifest.filenames.contains("not_a_video.mp4"));

    // Check that we can retrieve a feature for a corrupt file
    let feat = service.get_feature_by_identifier("corrupt_5.mp4").await;
    assert!(
        feat.is_some(),
        "Should still expose corrupt assets as features"
    );

    println!("Foundry: Media corruption handled gracefully.");
}

// #[tokio::test]
// fn test_sync_handles_malformed_files() {
//  set up file reader
//
//  make a file that's a page, but its filename is "page.mp4"
//  make a file that claims to be several gigabytes but is only 5kb
//  make a file with infinite recursion in one of its links
//
//  init sync
//
//  sync should try to create page.mp4, fail, handle gracefully, and we should not be able to find
//  page.mp4 in the pages table nor the video assets table
//
//  the "several gigabyte file" should parse just fine
//
//  the infinite recursion should also parse just fine with the link resolved
//
// }
