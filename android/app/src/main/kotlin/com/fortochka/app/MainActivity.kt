package com.fortochka.app

import android.Manifest
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.LaunchedEffect
import com.fortochka.app.camera.CameraService
import com.fortochka.app.camera.CameraStatus
import com.fortochka.app.viewer.ViewerStatus
import com.fortochka.app.viewer.WallpaperWorker
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.fortochka_mobile.DeviceConfig
import uniffi.fortochka_mobile.DeviceRole
import uniffi.fortochka_mobile.checkServer
import uniffi.fortochka_mobile.coreVersion
import uniffi.fortochka_mobile.pairWithCode
import uniffi.fortochka_mobile.registerCamera

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent { MaterialTheme { AppRoot() } }
    }
}

/** Rust-функции блокирующие — уводим их с main-потока. */
private suspend fun <T> onIo(block: () -> T): T = withContext(Dispatchers.IO) { block() }

@Composable
fun AppRoot() {
    val context = LocalContext.current
    var config by remember { mutableStateOf<DeviceConfig?>(null) }
    var loaded by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    LaunchedEffect(Unit) {
        config = onIo { ConfigStore.load(context) }
        loaded = true
    }
    if (!loaded) return

    val saveAndSet: (DeviceConfig) -> Unit = { new ->
        scope.launch {
            onIo { ConfigStore.save(context, new) }
            config = new
        }
    }
    val reset: () -> Unit = {
        CameraService.stop(context)
        WallpaperWorker.cancel(context)
        saveAndSet(ConfigStore.EMPTY)
    }

    when (config?.role) {
        DeviceRole.CAMERA -> CameraScreen(config!!, onReset = reset)
        DeviceRole.VIEWER -> ViewerScreen(config!!, onReset = reset)
        else -> SetupScreen(onDone = saveAndSet)
    }
}

@Composable
fun SetupScreen(onDone: (DeviceConfig) -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var serverUrl by remember { mutableStateOf("https://fortochka.fun") }
    var cameraName by remember { mutableStateOf("") }
    var pairingCode by remember { mutableStateOf("") }
    var status by remember { mutableStateOf("Rust-ядро на связи, версия ${coreVersion()}") }

    fun doRegisterCamera() {
        scope.launch {
            status = try {
                val reg = onIo { registerCamera(serverUrl, cameraName.ifBlank { "Камера" }) }
                onDone(
                    DeviceConfig(
                        serverUrl = serverUrl,
                        role = DeviceRole.CAMERA,
                        cameraId = reg.cameraId,
                        uploadToken = reg.uploadToken,
                        pairingCode = reg.pairingCode,
                        captureIntervalSecs = null,
                        viewToken = null,
                        cameraName = null,
                    ),
                )
                CameraService.start(context)
                "камера зарегистрирована"
            } catch (e: Exception) {
                "Ошибка: ${e.message}"
            }
        }
    }

    val permissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions(),
    ) { grants ->
        if (grants.values.all { it }) {
            doRegisterCamera()
        } else {
            status = "Без разрешения на камеру роль «камера» невозможна"
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text("Форточка", style = MaterialTheme.typography.headlineMedium)
        Text(status, style = MaterialTheme.typography.bodyMedium)

        OutlinedTextField(
            value = serverUrl,
            onValueChange = { serverUrl = it },
            label = { Text("Адрес сервера") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedButton(onClick = {
            scope.launch {
                status = try {
                    onIo { checkServer(serverUrl) }
                    "Сервер отвечает ✅"
                } catch (e: Exception) {
                    "Ошибка: ${e.message}"
                }
            }
        }) { Text("Проверить сервер") }

        Text("Этот телефон — камера:", style = MaterialTheme.typography.titleMedium)
        OutlinedTextField(
            value = cameraName,
            onValueChange = { cameraName = it },
            label = { Text("Название камеры (например, «Дача»)") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        Button(onClick = {
            val perms = buildList {
                add(Manifest.permission.CAMERA)
                if (Build.VERSION.SDK_INT >= 33) add(Manifest.permission.POST_NOTIFICATIONS)
            }
            permissionLauncher.launch(perms.toTypedArray())
        }) { Text("Стать камерой") }

        Text("Этот телефон — зритель:", style = MaterialTheme.typography.titleMedium)
        OutlinedTextField(
            value = pairingCode,
            onValueChange = { pairingCode = it },
            label = { Text("Код камеры") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        Button(onClick = {
            scope.launch {
                status = try {
                    val cam = onIo { pairWithCode(serverUrl, pairingCode) }
                    onDone(
                        DeviceConfig(
                            serverUrl = serverUrl,
                            role = DeviceRole.VIEWER,
                            cameraId = null,
                            uploadToken = null,
                            pairingCode = null,
                            captureIntervalSecs = null,
                            viewToken = cam.viewToken,
                            cameraName = cam.cameraName,
                        ),
                    )
                    // раз в 15 минут + первый кадр сразу
                    WallpaperWorker.schedule(context)
                    WallpaperWorker.refreshNow(context)
                    "Подключена камера «${cam.cameraName}» ✅"
                } catch (e: Exception) {
                    "Ошибка: ${e.message}"
                }
            }
        }) { Text("Подключиться к камере") }
    }
}

@Composable
fun CameraScreen(config: DeviceConfig, onReset: () -> Unit) {
    val context = LocalContext.current
    val status by CameraStatus.text.collectAsState()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text("Форточка — камера", style = MaterialTheme.typography.headlineMedium)
        Text("Код для подключения зрителей:", style = MaterialTheme.typography.bodyMedium)
        Text(
            config.pairingCode ?: "—",
            style = MaterialTheme.typography.displayMedium,
        )
        Text(status, style = MaterialTheme.typography.bodyLarge)

        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Button(onClick = { CameraService.start(context) }) { Text("Запустить") }
            OutlinedButton(onClick = { CameraService.stop(context) }) { Text("Остановить") }
        }

        // вендорские киллеры фоновых процессов — главный враг телефона-камеры
        OutlinedButton(onClick = {
            @Suppress("BatteryLife")
            context.startActivity(
                Intent(
                    Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS,
                    Uri.parse("package:${context.packageName}"),
                ),
            )
        }) { Text("Разрешить работу в фоне") }

        OutlinedButton(onClick = onReset) { Text("Сбросить настройки") }
    }
}

@Composable
fun ViewerScreen(config: DeviceConfig, onReset: () -> Unit) {
    val context = LocalContext.current
    val status by ViewerStatus.text.collectAsState()

    LaunchedEffect(Unit) { ViewerStatus.restore(context) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text("Форточка — зритель", style = MaterialTheme.typography.headlineMedium)
        Text(
            "Окно в «${config.cameraName ?: "камеру"}»: обои обновляются раз в 15 минут.",
            style = MaterialTheme.typography.bodyMedium,
        )
        Text(status, style = MaterialTheme.typography.bodyLarge)

        Button(onClick = { WallpaperWorker.refreshNow(context) }) {
            Text("Обновить обои сейчас")
        }
        OutlinedButton(onClick = onReset) { Text("Сбросить настройки") }
    }
}
