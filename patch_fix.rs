--- src/fix.rs
+++ src/fix.rs
@@ -118,7 +118,10 @@

         // Run cargo update
         println!("{}", "📦 Running cargo update...".dimmed());
-        let status = Command::new("cargo")
+
+        let cargo_bin = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
+
+        let status = Command::new(&cargo_bin)
             .arg("update")
             .current_dir(
                 manifest_path
