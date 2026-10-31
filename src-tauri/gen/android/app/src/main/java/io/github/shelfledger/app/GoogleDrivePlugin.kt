package io.github.shelfledger.app

import android.app.Activity
import androidx.activity.result.ActivityResult
import androidx.activity.result.IntentSenderRequest
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.android.gms.auth.api.identity.AuthorizationRequest
import com.google.android.gms.auth.api.identity.AuthorizationResult
import com.google.android.gms.auth.api.identity.Identity
import com.google.android.gms.common.api.Scope

/** Tokens return to Rust only; no token is persisted or exposed to the webview. */
@TauriPlugin
class GoogleDrivePlugin(private val activity: Activity) : Plugin(activity) {
    private val driveScope = "https://www.googleapis.com/auth/drive.appdata"

    @Command
    fun authorizeDrive(invoke: Invoke) {
        val request = AuthorizationRequest.builder()
            .setRequestedScopes(listOf(Scope(driveScope)))
            .build()
        Identity.getAuthorizationClient(activity).authorize(request)
            .addOnSuccessListener { result ->
                if (result.hasResolution()) {
                    val pending = result.pendingIntent
                    if (pending == null) {
                        invoke.reject("Google did not provide an authorization prompt")
                    } else {
                        try {
                            startIntentSenderForResult(invoke, IntentSenderRequest.Builder(pending).build(), "driveAuthorized")
                        } catch (_: Exception) {
                            invoke.reject("Could not open Google authorization")
                        }
                    }
                } else {
                    resolve(invoke, result)
                }
            }
            .addOnFailureListener { invoke.reject("Google Drive authorization failed") }
    }

    @ActivityCallback
    fun driveAuthorized(invoke: Invoke, result: ActivityResult) {
        if (result.resultCode != Activity.RESULT_OK || result.data == null) {
            invoke.reject("Google authorization cancelled")
            return
        }
        try {
            resolve(invoke, Identity.getAuthorizationClient(activity).getAuthorizationResultFromIntent(result.data))
        } catch (_: Exception) {
            invoke.reject("Google Drive authorization failed")
        }
    }

    private fun resolve(invoke: Invoke, result: AuthorizationResult) {
        val token = result.accessToken
        if (token.isNullOrEmpty() || !result.grantedScopes.contains(driveScope)) {
            invoke.reject("Google did not grant Drive access")
            return
        }
        val response = JSObject()
        response.put("accessToken", token)
        invoke.resolve(response)
    }
}
