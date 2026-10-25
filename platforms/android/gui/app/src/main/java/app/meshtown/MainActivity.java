package app.meshtown;

import android.app.Activity;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.net.VpnService;
import android.os.Build;
import android.os.Bundle;
import android.os.SystemClock;
import android.text.SpannableString;
import android.text.Spanned;
import android.text.style.ForegroundColorSpan;
import android.view.Menu;
import android.view.MenuItem;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.WebResourceError;
import android.webkit.WebResourceRequest;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.net.URLEncoder;

/** One screen: a Connect/Disconnect button, a hint and the node page in a WebView. */
public class MainActivity extends Activity {
    private static final int REQ_VPN = 1;
    private static final int REQ_NOTIFY = 2;
    private Button button;
    private AppMenu menu;
    private LinearLayout bar;
    private TextView hint;
    private WebView web;
    private boolean loaded;
    /** When the node last stopped answering; the splash message depends on how long ago. */
    private long waitingSince;
    private String splashText;
    private String pendingInvite;
    private ScanFlow scan;
    private final Runnable refresh = this::refresh;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        button = new Button(this);
        button.setOnClickListener(v -> toggle());
        menu = new AppMenu(this, button);
        hint = new TextView(this);
        hint.setText("Join or create a network first");
        hint.setGravity(android.view.Gravity.CENTER);
        hint.setMinHeight(Math.round(28 * getResources().getDisplayMetrics().density));
        web = new WebView(this);
        web.setWebViewClient(new WebViewClient() {
            @Override
            public boolean shouldOverrideUrlLoading(WebView v, WebResourceRequest r) {
                if (AppMenu.START_OVER_URL.equals(r.getUrl().toString())) {
                    menu.confirmStartOver();
                    return true;
                }
                if (InviteScan.URL.equals(r.getUrl().toString())) {
                    scan.start();
                    return true;
                }
                if (UpdateLink.matches(r.getUrl().toString())) {
                    UpdateInstaller.start(MainActivity.this,
                            UpdateLink.source(r.getUrl().toString(), BuildConfig.UPDATE_TEST_URL));
                    return true;
                }
                if ("https".equals(r.getUrl().getScheme())) {
                    startActivity(new android.content.Intent(android.content.Intent.ACTION_VIEW, r.getUrl()));
                    return true;
                }
                return false;
            }

            @Override
            public void onReceivedError(WebView v, WebResourceRequest r, WebResourceError e) {
                if (r.isForMainFrame() && NodeService.state.reachable) {
                    v.postDelayed(() -> v.loadUrl(NodeApi.pageUrl(MainActivity.this)), 1500);
                }
            }
        });
        web.getSettings().setJavaScriptEnabled(true);
        web.getSettings().setDomStorageEnabled(true);
        web.getSettings().setUserAgentString(web.getSettings().getUserAgentString() + " MeshTownApp/Android");
        root.addView(web, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));
        bar = new LinearLayout(this);
        bar.setOrientation(LinearLayout.VERTICAL);
        bar.addView(hint, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        bar.addView(button, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        root.addView(bar, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        root.setOnApplyWindowInsetsListener((v, insets) -> {
            v.setPadding(insets.getSystemWindowInsetLeft(), insets.getSystemWindowInsetTop(),
                    insets.getSystemWindowInsetRight(), insets.getSystemWindowInsetBottom());
            return insets;
        });
        // The root shrinks under adjustResize, so the visible frame is compared with the screen height.
        root.getViewTreeObserver().addOnGlobalLayoutListener(() -> {
            android.graphics.Rect frame = new android.graphics.Rect();
            root.getWindowVisibleDisplayFrame(frame);
            int screen = getResources().getDisplayMetrics().heightPixels;
            bar.setVisibility(KeyboardGuard.isOpen(screen, frame.height()) ? View.GONE : View.VISIBLE);
        });
        setContentView(root);
        waitingSince = SystemClock.elapsedRealtime();
        showSplash(Splash.STARTING);
        scan = new ScanFlow(this, text -> {
            pendingInvite = text;
            loadPage();
        });
        takeInvite(getIntent());
        NodeService.quitRequested = false;
        startForegroundService(new Intent(this, NodeService.class));
        refresh();
        UpdateInstaller.startIfRequested(this, getIntent());
    }

    @Override
    public boolean onCreateOptionsMenu(Menu menu) {
        this.menu.create(menu);
        return true;
    }

    @Override
    public boolean onPrepareOptionsMenu(Menu menu) {
        this.menu.prepare(menu);
        return super.onPrepareOptionsMenu(menu);
    }

    @Override
    public boolean onOptionsItemSelected(MenuItem item) {
        return menu.select(item) || super.onOptionsItemSelected(item);
    }

    /**
     * Back walks the page history while the page sits on a tab other than Home (tabs are address-hash
     * links, so each switch is a history entry); otherwise the app moves to the background, the node
     * and the VPN keep running.
     */
    @Override
    public void onBackPressed() {
        String url = web.getUrl();
        if (web.canGoBack() && url != null && url.contains("#")) {
            web.goBack();
            return;
        }
        moveTaskToBack(true);
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        takeInvite(intent);
        loadPage();
        UpdateInstaller.startIfRequested(this, intent);
    }

    /** Remembers the shared text when it is an invite; the secret is never logged. */
    private void takeInvite(Intent intent) {
        String text = intent == null ? null : intent.getStringExtra(Intent.EXTRA_TEXT);
        if (text == null && intent != null) {
            text = intent.getDataString();
        }
        if (text != null && text.trim().startsWith("mesh-invite")) {
            pendingInvite = text.trim();
        }
    }

    private void loadPage() {
        if (!NodeService.state.reachable) {
            return;
        }
        String url = NodeApi.pageUrl(this);
        if (pendingInvite != null) {
            try {
                url += "&invite=" + URLEncoder.encode(pendingInvite, "UTF-8");
            } catch (java.io.UnsupportedEncodingException ignored) {
                // UTF-8 is always supported
            }
            pendingInvite = null;
        }
        loaded = true;
        web.loadUrl(url);
    }

    private void showSplash(String text) {
        if (text.equals(splashText)) {
            return;
        }
        splashText = text;
        web.loadDataWithBaseURL(null, Splash.html(text), "text/html", "UTF-8", null);
    }

    /** Connect is green, Disconnect is grey, so the two states are told apart at a glance. */
    private void tintButton(boolean on, boolean enabled) {
        int color = !enabled ? 0xFFBDBDBD : on ? 0xFF616161 : 0xFF2E7D32;
        button.setBackgroundTintList(android.content.res.ColorStateList.valueOf(color));
        button.setTextColor(0xFFFFFFFF);
    }

    /** The line above the button; the dot, when there is one, is a coloured span, not a view. */
    private void showStrip(StatusText.Line line) {
        SpannableString text = new SpannableString(line.text);
        if (line.dotColor != 0) {
            text.setSpan(new ForegroundColorSpan(line.dotColor), 0, 1, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        }
        hint.setText(text);
    }

    private void refresh() {
        if (NodeService.quitRequested) {
            finishAndRemoveTask();
            return;
        }
        NodeState s = NodeService.state;
        boolean on = TunnelService.running;
        button.setText(on ? "Disconnect" : "Connect");
        boolean enabled = on || NodeService.canConnect;
        button.setEnabled(enabled);
        tintButton(on, enabled);
        showStrip(StatusText.strip(s, NodeService.canConnect, on));
        if (!s.reachable) {
            loaded = false;
            showSplash(Splash.message(SystemClock.elapsedRealtime() - waitingSince));
        } else if (!loaded) {
            splashText = null;
            waitingSince = SystemClock.elapsedRealtime();
            loadPage();
        }
        button.postDelayed(refresh, 2000);
    }

    @Override
    protected void onDestroy() {
        button.removeCallbacks(refresh);
        super.onDestroy();
    }

    private void toggle() {
        if (TunnelService.running) {
            Prefs.setVpnWanted(this, false);
            startService(new Intent(this, TunnelService.class).setAction(TunnelService.ACTION_STOP));
            return;
        }
        // A refusal does not block Connect: the answer, either way, continues into the VPN step.
        if (Build.VERSION.SDK_INT >= 33
                && checkSelfPermission("android.permission.POST_NOTIFICATIONS") != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[] {"android.permission.POST_NOTIFICATIONS"}, REQ_NOTIFY);
            return;
        }
        connect();
    }

    @Override
    public void onRequestPermissionsResult(int req, String[] perms, int[] results) {
        if (req == REQ_NOTIFY) {
            connect();
        }
    }

    private void connect() {
        Intent ask = VpnService.prepare(this);
        if (ask == null) {
            onActivityResult(REQ_VPN, RESULT_OK, null);
        } else {
            startActivityForResult(ask, REQ_VPN);
        }
    }

    @Override
    protected void onActivityResult(int req, int result, Intent data) {
        if (req == ScanFlow.REQ_SCAN) {
            scan.onResult(result);
            return;
        }
        if (req == REQ_VPN && result == RESULT_OK) {
            Prefs.setVpnWanted(this, true);
            startService(new Intent(this, TunnelService.class));
            BatteryPrompt.askOnce(this);
        }
    }
}
