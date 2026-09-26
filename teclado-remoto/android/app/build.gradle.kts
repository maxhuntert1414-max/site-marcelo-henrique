plugins {
    id("com.android.application")
}

android {
    namespace = "com.marcelohenrique.tecladoremoto"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.marcelohenrique.tecladoremoto"
        minSdk = 21
        targetSdk = 36
        versionCode = 1
        versionName = "1.0.0"
    }

    signingConfigs {
        // Chave de desenvolvimento versionada no repositório para que qualquer build
        // atualize o app instalado. Para publicar, defina as variáveis TRK_KEYSTORE*.
        create("sideload") {
            storeFile = file(System.getenv("TRK_KEYSTORE") ?: "../keystore/teclado-remoto-dev.jks")
            storePassword = System.getenv("TRK_KEYSTORE_PASSWORD") ?: "tecladoremoto"
            keyAlias = System.getenv("TRK_KEY_ALIAS") ?: "tecladoremoto"
            keyPassword = System.getenv("TRK_KEY_PASSWORD") ?: "tecladoremoto"
        }
    }

    buildTypes {
        getByName("release") {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.getByName("sideload")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }

    packaging {
        resources {
            excludes += listOf("kotlin/**", "META-INF/*.kotlin_module", "META-INF/*.version", "DebugProbesKt.bin")
        }
    }

    dependenciesInfo {
        includeInApk = false
        includeInBundle = false
    }

    lint {
        abortOnError = true
        checkReleaseBuilds = true
        // App só em português; targetSdk fica no 36 de propósito (o 37 muda regras de acesso à rede local).
        disable += listOf("SetTextI18n", "HardcodedText", "OldTargetApi", "GradleDependency", "UseRequiresApi")
    }
}

dependencies {
    implementation(project(":core"))
}
