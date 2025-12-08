// -*- fill-column: 80; -*-

fn unsound_feature_warn(feature: &str, explanation: &str) {
    println!(
        "cargo:warning=\n\
         cargo:warning=--------------------------------------------------\n\
         cargo:warning=\n\
         cargo:warning=!!!DANGER!!!\n\
         cargo:warning=\n\
         cargo:warning=OMNIGLOT {} ARE\n\
         cargo:warning=DISABLED. THIS IS UNSOUND.\n\
         cargo:warning=\n\
         cargo:warning=The feature\n\
         cargo:warning=     \"{}\"\n\
         cargo:warning=is only intended for benchmark purposes. Disable it!\n\
         cargo:warning=\n\
         cargo:warning=!!!DANGER!!!\n\
         cargo:warning=\n\
         cargo:warning=--------------------------------------------------\n\
         cargo:warning=\n\
         ",
        explanation, feature,
    );
}

fn main() {
    if cfg!(feature = "disable_upgrade_checks") {
        unsound_feature_warn(
            "disable_upgrade_checks",
            "FOREIGN MEMORY REFERENCE UPGRADE CHECKS",
        );
    }

    if cfg!(feature = "disable_validation_checks") {
        unsound_feature_warn(
            "disable_validation_checks",
            "FOREIGN MEMORY REFERENCE VALIDATION CHECKS",
        );
    }
}
