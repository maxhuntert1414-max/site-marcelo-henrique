import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("org.jetbrains.kotlin.jvm")
}

java {
    sourceCompatibility = JavaVersion.VERSION_11
    targetCompatibility = JavaVersion.VERSION_11
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_11)
    }
}

dependencies {
    testImplementation(kotlin("test"))
}

tasks.test {
    useJUnitPlatform()
    // Teste ponta a ponta contra o servidor do PC em modo headless (tools/e2e.sh).
    for (name in listOf("TRK_E2E_PORT", "TRK_E2E_DISCOVERY_PORT", "TRK_SMOKE_HOST", "TRK_SMOKE_PORT")) {
        environment(name, providers.environmentVariable(name).orElse("").get())
    }
    testLogging {
        events("failed")
        showStandardStreams = System.getenv("TRK_SMOKE_HOST") != null
        exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
    }
}
