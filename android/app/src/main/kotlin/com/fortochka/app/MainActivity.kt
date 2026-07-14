package com.fortochka.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.coroutines.launch
import uniffi.fortochka_mobile.checkServer
import uniffi.fortochka_mobile.coreVersion
import uniffi.fortochka_mobile.pairWithCode

/**
 * Скелет-экран Milestone 1: доказывает работу всей цепочки
 * Kotlin → UniFFI → Rust-ядро → сервер. Роли «камера» и «зритель»
 * появятся в следующих вехах.
 */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent { MaterialTheme { FortochkaScreen() } }
    }
}

/** Rust-функции блокирующие — уводим их с main-потока. */
private suspend fun <T> onIo(block: () -> T): T = withContext(Dispatchers.IO) { block() }

@Composable
fun FortochkaScreen() {
    val scope = rememberCoroutineScope()
    var serverUrl by remember { mutableStateOf("https://fortochka.fun") }
    var pairingCode by remember { mutableStateOf("") }
    var status by remember { mutableStateOf("Rust-ядро на связи, версия ${coreVersion()}") }

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
        Button(onClick = {
            scope.launch {
                status = try {
                    onIo { checkServer(serverUrl) }
                    "Сервер отвечает ✅"
                } catch (e: Exception) {
                    "Ошибка: ${e.message}"
                }
            }
        }) { Text("Проверить сервер") }

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
                    "Подключена камера «${cam.cameraName}» ✅"
                } catch (e: Exception) {
                    "Ошибка: ${e.message}"
                }
            }
        }) { Text("Подключиться к камере") }
    }
}
