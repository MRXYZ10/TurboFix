package com.plugin.mobile_boost

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.wifi.WifiInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.provider.Settings
import org.json.JSONObject
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import app.tauri.plugin.Invoke

@InvokeArg
class PingArgs {
  var value: String? = null
}

@TauriPlugin
class ExamplePlugin(private val activity: Activity): Plugin(activity) {
    private val implementation = Example()

    @Command
    fun ping(invoke: Invoke) {
        val args = invoke.parseArgs(PingArgs::class.java)

        val ret = JSObject()
        ret.put("value", implementation.pong(args.value ?: "default value :("))
        invoke.resolve(ret)
    }

    @Command
    fun wifiInfo(invoke: Invoke) {
        val context = activity.applicationContext
        val wifiManager = context.getSystemService(Context.WIFI_SERVICE) as? WifiManager
        val connectivityManager = context.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager
        val network = connectivityManager?.activeNetwork
        val capabilities = if (network != null) connectivityManager?.getNetworkCapabilities(network) else null
        val connected = capabilities?.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) == true
        @Suppress("DEPRECATION")
        val legacyInfo = if (connected && Build.VERSION.SDK_INT < Build.VERSION_CODES.S) wifiManager?.connectionInfo else null
        val info: WifiInfo? = if (!connected) {
            null
        } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            capabilities?.transportInfo as? WifiInfo
        } else {
            legacyInfo
        }
        val wifiEnabled = try {
            wifiManager?.isWifiEnabled ?: false
        } catch (_: SecurityException) {
            false
        }
        val rssi = info?.rssi?.takeIf { it > -127 && it <= 0 }
        val rxSpeed = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            info?.rxLinkSpeedMbps?.takeIf { it > 0 }
        } else null
        val txSpeed = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            info?.txLinkSpeedMbps?.takeIf { it > 0 }
        } else null
        val frequency = info?.frequency?.takeIf { it > 0 }

        val ret = JSObject()
        ret.put("supported", true)
        ret.put("wifiEnabled", wifiEnabled)
        ret.put("connected", connected)
        ret.put("rssiDbm", rssi ?: JSONObject.NULL)
        ret.put("rxLinkSpeedMbps", rxSpeed ?: JSONObject.NULL)
        ret.put("txLinkSpeedMbps", txSpeed ?: JSONObject.NULL)
        ret.put("frequencyMhz", frequency ?: JSONObject.NULL)
        ret.put(
            "note",
            when {
                !wifiEnabled -> "Wi-Fi is turned off. Open Android Wi-Fi settings to turn it on."
                !connected -> "No active Wi-Fi connection was detected. Connect to Wi-Fi and refresh."
                info == null -> "Android did not expose current Wi-Fi details on this device."
                else -> "Values are read from Android. Link speed is the phone-to-router link rate, not internet throughput."
            }
        )
        invoke.resolve(ret)
    }

    @Command
    fun openWifiSettings(invoke: Invoke) {
        val intent = Intent(Settings.ACTION_WIFI_SETTINGS)
        activity.startActivity(intent)
        val ret = JSObject()
        ret.put("ok", true)
        invoke.resolve(ret)
    }
}
