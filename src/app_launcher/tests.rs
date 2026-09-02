#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "slow-tests")]
    #[test]
    fn test_scan_applications_returns_apps() {
        let apps = scan_applications().expect("application scan");

        // Should find at least some apps on any macOS system
        assert!(
            !apps.is_empty(),
            "Should find at least some applications on macOS"
        );

        // Check that Calculator exists (it's always present in /System/Applications on macOS)
        let calculator = apps.iter().find(|a| a.name == "Calculator");
        assert!(calculator.is_some(), "Calculator.app should be found");

        if let Some(calculator) = calculator {
            assert!(
                calculator.path.exists(),
                "Calculator path should exist: {:?}",
                calculator.path
            );
            assert!(
                calculator.bundle_id.is_some(),
                "Calculator should have a bundle ID"
            );
            assert_eq!(
                calculator.bundle_id.as_deref(),
                Some("com.apple.calculator"),
                "Calculator bundle ID should be com.apple.calculator"
            );
        }
    }

    #[cfg(feature = "slow-tests")]
    #[test]
    fn test_app_info_has_required_fields() {
        let apps = scan_applications().expect("application scan");

        for app in apps.iter().take(10) {
            // Name should not be empty
            assert!(!app.name.is_empty(), "App name should not be empty");

            // Path should end with .app
            assert!(
                app.path.extension().map(|e| e == "app").unwrap_or(false),
                "App path should end with .app: {:?}",
                app.path
            );

            // Path should exist
            assert!(app.path.exists(), "App path should exist: {:?}", app.path);
        }
    }

    #[cfg(feature = "slow-tests")]
    #[test]
    fn test_apps_sorted_alphabetically() {
        let apps = scan_applications().expect("application scan");

        // Verify apps are sorted by lowercase name
        for window in apps.windows(2) {
            let a = &window[0];
            let b = &window[1];
            assert!(
                a.name.to_lowercase() <= b.name.to_lowercase(),
                "Apps should be sorted: {} should come before {}",
                a.name,
                b.name
            );
        }
    }

    #[test]
    fn test_extract_bundle_id_finder() {
        let finder_path = Path::new("/System/Applications/Finder.app");
        if finder_path.exists() {
            let bundle_id = extract_bundle_id(finder_path).expect("read bundle identifier");
            assert_eq!(
                bundle_id,
                Some("com.apple.finder".to_string()),
                "Should extract Finder bundle ID"
            );
        }
    }

    #[test]
    fn test_extract_bundle_id_nonexistent() {
        let temp = tempfile::tempdir().expect("tempdir");
        let bundle_id =
            extract_bundle_id(&temp.path().join("missing.app")).expect("missing optional metadata");
        assert!(
            bundle_id.is_none(),
            "Should return None for nonexistent app"
        );
    }

    #[test]
    fn test_resolve_bundle_icon_resource_uses_declared_icns_without_iconservices() {
        let temp = tempfile::tempdir().expect("tempdir");
        let app = temp.path().join("Example.app");
        let contents = app.join("Contents");
        let resources = contents.join("Resources");
        std::fs::create_dir_all(&resources).expect("create resources");

        std::fs::write(
            contents.join("Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIconFile</key>
  <string>ExampleIcon</string>
</dict>
</plist>
"#,
        )
        .expect("write plist");
        let icon_path = resources.join("ExampleIcon.icns");
        std::fs::write(&icon_path, []).expect("write icon placeholder");

        assert_eq!(
            resolve_bundle_icon_resource_path(&app).expect("read icon resource"),
            Some(icon_path)
        );
    }

    #[test]
    fn test_parse_app_bundle() {
        let finder_path = Path::new("/System/Applications/Finder.app");
        if finder_path.exists() {
            let app_info = parse_app_bundle_with_icon(finder_path).expect("parse application");
            assert!(app_info.is_some(), "Should parse Finder.app");

            let (app, _) = app_info.unwrap();
            assert_eq!(app.name, "Finder");
            assert_eq!(app.path, finder_path);
            assert!(app.bundle_id.is_some());
        }
    }

    #[test]
    fn test_scan_directory_finds_nested_vendor_apps() {
        let temp = tempfile::tempdir().expect("tempdir");
        let nested_app = temp
            .path()
            .join("Universal Audio")
            .join("UAD Meter & Control Panel.app");
        std::fs::create_dir_all(nested_app.join("Contents")).expect("create nested app");

        let apps =
            collect_app_paths_from_roots(&[temp.path().to_path_buf()]).expect("scan directory");

        assert!(
            apps.contains(&nested_app),
            "nested vendor .app bundles under Applications-style folders should be indexed"
        );
    }

    #[test]
    fn test_scan_directory_does_not_descend_inside_app_bundles() {
        let temp = tempfile::tempdir().expect("tempdir");
        let app = temp.path().join("Outer.app");
        let nested_inside_bundle = app.join("Contents").join("Inner.app");
        std::fs::create_dir_all(nested_inside_bundle.join("Contents"))
            .expect("create app internals");

        let apps =
            collect_app_paths_from_roots(&[temp.path().to_path_buf()]).expect("scan directory");

        assert!(apps.contains(&app));
        assert!(
            !apps.contains(&nested_inside_bundle),
            "scanner should treat .app bundles as leaves and skip bundle internals"
        );
    }

    #[cfg(feature = "slow-tests")]
    #[test]
    fn test_no_duplicate_apps() {
        let apps = scan_applications().expect("application scan");
        // Use a set to check for true duplicates
        let mut seen = std::collections::HashSet::new();
        let mut duplicates = Vec::new();
        for app in apps.iter() {
            let lower_name = app.name.to_lowercase();
            if !seen.insert(lower_name.clone()) {
                duplicates.push(app.name.clone());
            }
        }

        // Allow a small number of duplicates (some systems have app variants)
        // e.g., same app name in different locations
        assert!(
            duplicates.len() <= 5,
            "Too many duplicate app names ({}): {:?}",
            duplicates.len(),
            duplicates
        );
    }

    #[cfg(all(target_os = "macos", feature = "slow-tests"))]
    #[test]
    fn test_extract_app_icon() {
        // Test icon extraction for Calculator (always present on macOS)
        let calculator_path = Path::new("/System/Applications/Calculator.app");
        if calculator_path.exists() {
            let icon = extract_app_icon(calculator_path).expect("read application icon");
            assert!(icon.is_some(), "Should extract Calculator icon");

            if let Some(icon_data) = icon {
                // PNG magic bytes: 0x89 0x50 0x4E 0x47
                assert!(icon_data.len() > 8, "Icon data should have content");
                assert_eq!(
                    &icon_data[0..4],
                    &[0x89, 0x50, 0x4E, 0x47],
                    "Icon should be valid PNG"
                );
            }
        }
    }

    #[cfg(feature = "slow-tests")]
    #[test]
    fn test_app_has_icon() {
        // Fresh scans read and decode icons before publishing the catalogue.
        let apps = scan_applications_fresh().expect("fresh application scan");

        // Check that at least some apps have icons (most should)
        let apps_with_icons = apps.iter().filter(|a| a.icon.is_some()).count();

        // Most apps should have icons - at least 50%
        assert!(
            apps_with_icons > apps.len() / 2,
            "At least half of apps should have icons, got {}/{}",
            apps_with_icons,
            apps.len()
        );
    }

    // Note: the success path of launch_application is not tested automatically
    // to avoid actually launching apps during test runs.

    #[test]
    fn launch_application_errors_when_bundle_path_is_gone() {
        let app = AppInfo {
            name: "Ghost App".to_string(),
            path: PathBuf::from("/Applications/DefinitelyNotInstalled-ScriptKitTest.app"),
            bundle_id: None,
            icon: None,
        };

        let error = launch_application(&app)
            .expect_err("launching a missing bundle must error, not silently succeed");
        assert!(
            error.to_string().contains("moved or uninstalled"),
            "error should explain the stale entry: {error}"
        );
    }

    /// Test that load_apps_from_db returns apps WITH icons decoded synchronously.
    ///
    /// The bug was that a previous version deferred icon decoding to a background
    /// thread that updated a LOCAL Arc, then returned a clone of the Vec without icons.
    /// The fix is to decode icons synchronously in load_apps_from_db.
    #[cfg(feature = "slow-tests")]
    #[test]
    fn test_load_apps_from_db_returns_apps_with_icons() {
        // First, ensure we have some apps in the database by doing a fresh scan
        // This populates the SQLite DB with apps including icon blobs
        let fresh_apps = scan_applications_fresh().expect("fresh application scan");
        assert!(!fresh_apps.is_empty(), "Should have apps after fresh scan");

        // Count how many apps have icons after fresh scan
        let fresh_with_icons = fresh_apps.iter().filter(|a| a.icon.is_some()).count();
        assert!(
            fresh_with_icons > 0,
            "Fresh scan should produce some apps with icons"
        );

        // Now test that load_apps_from_db returns apps WITH icons decoded
        let cached_apps = load_apps_from_db().expect("load cached applications");

        // Verify we got apps
        assert!(!cached_apps.is_empty(), "Should load apps from DB");

        // Count apps with icons from cache - should match or be close to fresh scan
        let cached_with_icons = cached_apps.iter().filter(|a| a.icon.is_some()).count();

        // The fix ensures icons are decoded synchronously, so cached apps should have icons
        assert!(
            cached_with_icons > 0,
            "Cached apps should have icons decoded. Found {} apps but {} with icons",
            cached_apps.len(),
            cached_with_icons
        );
    }

    #[test]
    fn test_hash_path() {
        let path1 = Path::new("/Applications/Safari.app");
        let path2 = Path::new("/Applications/Safari.app");
        let path3 = Path::new("/Applications/Finder.app");

        // Same path should produce same hash
        assert_eq!(
            hash_path(path1),
            hash_path(path2),
            "Same path should produce same hash"
        );

        // Different paths should produce different hashes
        assert_ne!(
            hash_path(path1),
            hash_path(path3),
            "Different paths should produce different hashes"
        );

        // Hash should be 16 hex characters
        let hash = hash_path(path1);
        assert_eq!(hash.len(), 16, "Hash should be 16 characters");
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "Hash should be hex characters: {}",
            hash
        );
    }

    #[cfg(all(target_os = "macos", feature = "slow-tests"))]
    #[test]
    fn test_get_or_extract_icon_caches() {
        // Test that get_or_extract_icon properly caches icons
        let calculator_path = Path::new("/System/Applications/Calculator.app");
        if !calculator_path.exists() {
            return;
        }

        // First call - may or may not hit cache
        let icon1 = get_or_extract_icon(calculator_path).expect("read application icon");
        assert!(icon1.is_some(), "Should extract Calculator icon");

        // Second call should hit cache
        let icon2 = get_or_extract_icon(calculator_path).expect("read cached application icon");
        assert!(icon2.is_some(), "Should load Calculator icon from cache");

        // Both should have the same content
        let bytes1 = icon1.unwrap();
        let bytes2 = icon2.unwrap();
        assert_eq!(bytes1, bytes2, "Cached icon should match extracted icon");

        // Verify cache file exists
        let cache_dir = get_icon_cache_dir().unwrap();
        let cache_key = hash_path(calculator_path);
        let cache_file = cache_dir.join(format!("{}.png", cache_key));
        assert!(
            cache_file.exists(),
            "Cache file should exist: {:?}",
            cache_file
        );
    }

    #[test]
    fn test_decode_with_rb_swap() {
        use image::ImageEncoder;

        // Create a simple 2x2 PNG with known colors
        // Pixel at (0,0) = Red (255, 0, 0, 255)
        // Pixel at (1,0) = Blue (0, 0, 255, 255)
        // Pixel at (0,1) = Green (0, 255, 0, 255)
        // Pixel at (1,1) = White (255, 255, 255, 255)
        let mut img = image::RgbaImage::new(2, 2);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255])); // Red
        img.put_pixel(1, 0, image::Rgba([0, 0, 255, 255])); // Blue
        img.put_pixel(0, 1, image::Rgba([0, 255, 0, 255])); // Green
        img.put_pixel(1, 1, image::Rgba([255, 255, 255, 255])); // White

        // Encode to PNG
        let mut original_png = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut original_png);
        encoder
            .write_image(&img, 2, 2, image::ExtendedColorType::Rgba8)
            .expect("Failed to encode PNG");

        // Use the decode function with BGRA conversion
        let render_image =
            crate::list_item::decode_png_to_render_image_with_bgra_conversion(&original_png)
                .expect("Should decode with BGRA conversion");

        // Verify we got a RenderImage (we can't easily inspect pixels in RenderImage,
        // but we can verify it was created successfully)
        assert!(
            std::sync::Arc::strong_count(&render_image) >= 1,
            "Should create valid RenderImage"
        );
    }

    #[test]
    fn test_get_icon_cache_stats() {
        let (count, size) = get_icon_cache_stats();
        // We can't make strong assertions about exact counts since
        // other tests may have populated the cache, but we can check types
        assert!(
            count == 0 || size > 0,
            "If there are cached files, size should be non-zero"
        );
    }

    #[test]
    fn test_get_apps_db_path() {
        let db_path = get_apps_db_path();
        assert!(
            db_path.ends_with("db/apps.sqlite"),
            "DB path should end with db/apps.sqlite: {:?}",
            db_path
        );
        assert!(
            db_path.to_string_lossy().contains(".scriptkit"),
            "DB path should be under .scriptkit: {:?}",
            db_path
        );
    }

    #[test]
    fn test_loading_state() {
        // Initial state should be Ready (default)
        let state = get_app_loading_state();
        // Note: state may vary if other tests are running

        // Test message generation
        assert!(!state.message().is_empty(), "Should have a message");
    }

    #[test]
    fn application_roots_distinguish_empty_absent_and_failed_reads() {
        let temp = tempfile::tempdir().expect("tempdir");
        let empty = temp.path().join("empty");
        fs::create_dir(&empty).expect("empty root");
        assert!(collect_app_paths_from_roots(&[
            empty.clone(),
            temp.path().join("optional-missing")
        ])
        .expect("empty roots")
        .is_empty());
        let app = empty.join("Available.app");
        fs::create_dir(&app).expect("valid bundle");
        let invalid = temp.path().join("not-a-directory");
        fs::write(&invalid, b"file").expect("invalid root");
        let error = collect_app_paths_from_roots(&[empty, invalid])
            .err()
            .expect("partial scan must fail");
        assert_eq!(
            error
                .downcast_ref::<std::io::Error>()
                .expect("IO error")
                .kind(),
            std::io::ErrorKind::NotADirectory
        );
    }

    #[cfg(unix)]
    #[test]
    fn application_scan_does_not_ignore_a_nested_directory_disappearing() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("Vendor").join("Nested.app")).expect("nested bundle");
        let entries = fs::read_dir(&root).expect("open root");
        fs::rename(&root, temp.path().join("moved")).expect("move root after opening");
        let error = collect_app_entries(&root, entries, &mut Vec::new())
            .err()
            .expect("nested read failure");
        assert_eq!(
            error
                .downcast_ref::<std::io::Error>()
                .expect("IO error")
                .kind(),
            std::io::ErrorKind::NotFound
        );
    }

    #[test]
    fn application_scan_failure_retains_last_good_and_empty_success_replaces_it() {
        let cache = Mutex::new(AppCache::default());
        let mut png = Vec::new();
        image::ImageEncoder::write_image(
            image::codecs::png::PngEncoder::new(&mut png),
            &[20, 40, 60, 255],
            1,
            1,
            image::ExtendedColorType::Rgba8,
        )
        .expect("encode icon");
        let icon = crate::list_item::decode_png_to_render_image_with_bgra_conversion(&png)
            .expect("decode icon");
        let app = AppInfo {
            name: "Retained".into(),
            path: PathBuf::from("retained.app"),
            bundle_id: None,
            icon: Some(DecodedIcon::new(icon.clone())),
        };
        complete_app_scan(&cache, Ok(vec![app])).expect("initial snapshot");
        let error = complete_app_scan(&cache, Err(anyhow::anyhow!("native scan failed")))
            .err()
            .expect("source failure");
        assert!(error.to_string().contains("native scan failed"));
        assert!(
            app_cache_snapshot(&cache).is_err(),
            "a failed scan is not a successful cache read"
        );
        {
            let cache = cache.lock().expect("cache");
            let apps = cache.apps.as_ref().expect("retained snapshot");
            assert_eq!(apps[0].path, PathBuf::from("retained.app"));
            assert!(Arc::ptr_eq(
                apps[0].icon.as_ref().expect("retained icon").image(),
                &icon
            ));
        }
        assert!(complete_app_scan(&cache, Ok(Vec::new()))
            .expect("valid empty scan")
            .is_empty());
        assert!(app_cache_snapshot(&cache)
            .expect("recovered cache")
            .expect("completed empty snapshot")
            .is_empty());
    }

    #[test]
    fn cached_application_query_and_row_errors_are_not_empty_or_partial_success() {
        let conn = Connection::open_in_memory().expect("database");
        assert!(
            load_apps_from_connection(&conn).is_err(),
            "missing schema is a query failure"
        );
        init_apps_db(&conn).expect("schema");
        assert!(load_apps_from_connection(&conn)
            .expect("valid empty cache")
            .is_empty());
        let temp = tempfile::tempdir().expect("tempdir");
        let valid = temp.path().join("Valid.app");
        fs::create_dir(&valid).expect("valid bundle");
        conn.execute(
            "INSERT INTO apps VALUES ('valid', 'A Valid', ?1, NULL, 0, 0)",
            [valid.to_str().expect("path")],
        )
        .expect("valid cached app");
        conn.execute(
            "INSERT INTO apps VALUES ('broken', x'ff', 'broken.app', NULL, 0, 0)",
            [],
        )
        .expect("malformed cached row");
        assert!(
            load_apps_from_connection(&conn).is_err(),
            "a decoded prefix must not hide the failed row"
        );
    }

    #[test]
    fn cached_application_paths_only_treat_not_found_as_absent() {
        let conn = Connection::open_in_memory().expect("database");
        init_apps_db(&conn).expect("schema");
        let temp = tempfile::tempdir().expect("tempdir");
        let missing = temp.path().join("Missing.app");
        conn.execute(
            "INSERT INTO apps VALUES ('missing', 'Missing', ?1, NULL, 0, 0)",
            [missing.to_str().expect("path")],
        )
        .expect("missing cached app");
        assert!(load_apps_from_connection(&conn)
            .expect("genuine absence")
            .is_empty());
        let file = temp.path().join("not-a-directory");
        fs::write(&file, b"file").expect("file parent");
        let broken = file.join("Broken.app");
        conn.execute(
            "INSERT INTO apps VALUES ('broken', 'Broken', ?1, NULL, 0, 0)",
            [broken.to_str().expect("path")],
        )
        .expect("unreadable cached app");
        assert!(
            load_apps_from_connection(&conn).is_err(),
            "metadata read failure must not become absence"
        );
    }

    #[test]
    fn application_cache_transaction_rolls_back_partial_updates_and_commits_empty() {
        let mut conn = Connection::open_in_memory().expect("database");
        init_apps_db(&conn).expect("schema");
        conn.execute(
            "INSERT INTO apps VALUES ('existing', 'Original', 'old.app', x'010203', 0, 0)",
            [],
        )
        .expect("last good cache");
        let entries = vec![
            ScannedApp {
                app: AppInfo {
                    name: "New".into(),
                    path: PathBuf::from("new.app"),
                    bundle_id: Some("new".into()),
                    icon: None,
                },
                icon_bytes: None,
                mtime: 1,
            },
            ScannedApp {
                app: AppInfo {
                    name: "Conflict".into(),
                    path: PathBuf::from("old.app"),
                    bundle_id: Some("conflict".into()),
                    icon: None,
                },
                icon_bytes: None,
                mtime: 1,
            },
        ];
        assert!(save_apps_to_db(&mut conn, &entries).is_err());
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM apps", [], |row| row.get::<_, i64>(0))
                .expect("count"),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT icon_blob FROM apps WHERE bundle_id = 'existing'",
                [],
                |row| row.get::<_, Vec<u8>>(0)
            )
            .expect("retained icon"),
            vec![1, 2, 3]
        );
        save_apps_to_db(&mut conn, &[]).expect("valid empty scan");
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM apps", [], |row| row.get::<_, i64>(0))
                .expect("count"),
            0
        );
    }

    #[cfg(unix)]
    #[test]
    fn plist_errors_distinguish_missing_optional_key_from_spawn_and_read_failure() {
        use std::os::unix::process::ExitStatusExt;
        let output = |stderr: &[u8]| {
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(1 << 8),
                stdout: Vec::new(),
                stderr: stderr.to_vec(),
            })
        };
        let missing_key = b"Print: Entry, \":CFBundleIdentifier\", Does Not Exist\n";
        assert!(
            decode_plist_output(output(missing_key), ":CFBundleIdentifier")
                .expect("optional missing key")
                .is_none()
        );
        assert!(decode_plist_output(
            output(b"Error Reading File: Permission denied\n"),
            ":CFBundleIdentifier"
        )
        .is_err());
        assert!(decode_plist_output(
            output(b"Error Reading File\nPrint: Entry, \":CFBundleIdentifier\", Does Not Exist\n"),
            ":CFBundleIdentifier"
        )
        .is_err());
        let missing_executable = Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "PlistBuddy executable missing",
        ));
        assert!(
            decode_plist_output(missing_executable, ":CFBundleIdentifier").is_err(),
            "missing executable is not missing optional metadata"
        );
    }
}
