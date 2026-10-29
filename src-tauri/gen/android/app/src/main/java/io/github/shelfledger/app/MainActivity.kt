package io.github.shelfledger.app

import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import androidx.activity.OnBackPressedCallback
import android.webkit.WebView
import android.view.View
import android.graphics.Color
import androidx.activity.SystemBarStyle
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  private var appWebView: WebView? = null

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    appWebView = webView
    // Consume native insets once so older WebViews and keyboard resize do not
    // put controls behind the status bar, cutout, navigation bar or IME.
    val content = findViewById<View>(android.R.id.content)
    content.setBackgroundColor(Color.rgb(19, 21, 20))
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val safe = insets.getInsets(WindowInsetsCompat.Type.systemBars() or
        WindowInsetsCompat.Type.displayCutout() or WindowInsetsCompat.Type.ime())
      view.setPadding(safe.left, safe.top, safe.right, safe.bottom)
      WindowInsetsCompat.CONSUMED
    }
    ViewCompat.requestApplyInsets(content)
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge(
      statusBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
      navigationBarStyle = SystemBarStyle.dark(Color.rgb(19, 21, 20))
    )
    super.onCreate(savedInstanceState)
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        val webView = appWebView ?: return finish()
        webView.evaluateJavascript("""
          (() => {
            if (document.querySelector('[role="listbox"][data-state="open"]')) {
              document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
              return true;
            }
            const close = Array.from(document.querySelectorAll('[role="dialog"][data-state="open"]'))
              .reverse().map(dialog => dialog.querySelector('[data-dialog-close]')).find(Boolean);
            if (close) { close.click(); return true; }
            return !window.dispatchEvent(new Event('shelfledger:back', { cancelable: true }));
          })()
        """.trimIndent()) { handled -> if (handled != "true") finish() }
      }
    })
  }
}
