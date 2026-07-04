# Add project specific ProGuard rules here.
# By default, the flags in this file are appended to flags specified
# in the SDK tools.

# Tink crypto library depends on error_prone_annotations at compile time
# but doesn't bundle them at runtime — keep to avoid R8 resolution failures.
-keep class com.google.errorprone.annotations.** { *; }
-dontwarn com.google.errorprone.annotations.**
