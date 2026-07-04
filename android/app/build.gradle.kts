plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "com.fortochka.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.fortochka.app"
        // minSdk 24: роль камеры рассчитана на старые телефоны
        minSdk = 24
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        compose = true
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.material3)
    implementation(libs.compose.ui)
    // JNA нужен сгенерированным UniFFI-биндингам (мост Kotlin → libfortochka_mobile.so)
    implementation("net.java.dev.jna:jna:${libs.versions.jna.get()}@aar")
}
