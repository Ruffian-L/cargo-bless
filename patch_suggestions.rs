--- src/suggestions.rs
+++ src/suggestions.rs
@@ -19,10 +19,8 @@
     pub fn is_auto_fixable(&self) -> bool {
         matches!(
             self.kind,
             SuggestionKind::StdReplacement
-                | SuggestionKind::Unmaintained
-                | SuggestionKind::FeatureOptimization
+                | SuggestionKind::Unmaintained
         )
     }
 }
