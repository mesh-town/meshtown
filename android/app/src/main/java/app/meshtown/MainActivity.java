package app.meshtown;

import android.app.Activity;
import android.app.ActivityManager;
import android.app.AlertDialog;
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
    private static final int MENU_QUIT = 1;
    private static final int MENU_START_OVER = 2;
    /** The page asks for the start-over dialog by navigating here. */
    private static final String START_OVER_URL = "meshtown-app://start-over";
    private static final int KEYBOARD_MIN_DP = 120;
    private static final long WIPE_DELAY_MS = 1500;
    private Button button;
    private LinearLayout bar;
    private TextView hint;
    private WebView web;
    private boolean loaded;
    /** When the node last stopped answering; the splash message depends on how long ago. */
    private long waitingSince;
    private String splashText;
    private String pendingInvite;
    private final Runnable refresh = this::refresh;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        button = new Button(this);
        button.setOnClickListener(v -> toggle());
        hint = new TextView(this);
        hint.setText("Join or create a network first");
        hint.setGravity(android.view.Gravity.CENTER);
        hint.setMinHeight(Math.round(28 * getResources().getDisplayMetrics().density));
        web = new WebView(this);
        web.setWebViewClient(new WebViewClient() {
            @Override
            public boolean shouldOverrideUrlLoading(WebView v, WebResourceRequest r) {
                if (START_OVER_URL.equals(r.getUrl().toString())) {
                    confirmStartOver();
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
            int bottom = insets.getSystemWindowInsetBottom();
            v.setPadding(0, 0, 0, bottom);
            // Navigation bars are below this height; only an on-screen keyboard is taller.
            bar.setVisibility(bottom > KEYBOARD_MIN_DP * getResources().getDisplayMetrics().density
                    ? View.GONE : View.VISIBLE);
            return insets;
        });
        setContentView(root);
        waitingSince = SystemClock.elapsedRealtime();
        showSplash(Splash.STARTING);
        takeInvite(getIntent());
        NodeService.quitRequested = false;
        startForegroundService(new Intent(this, NodeService.class));
        refresh();
    }

    @Override
    public boolean onCreateOptionsMenu(Menu menu) {
        menu.add(Menu.NONE, MENU_QUIT, Menu.NONE, "Quit Mesh Town");
        menu.add(Menu.NONE, MENU_START_OVER, Menu.NONE, StatusText.resetMenuTitle(NodeService.state));
        return true;
    }

    @Override
    public boolean onPrepareOptionsMenu(Menu menu) {
        menu.findItem(MENU_START_OVER).setTitle(StatusText.resetMenuTitle(NodeService.state));
        return super.onPrepareOptionsMenu(menu);
    }

    @Override
    public boolean onOptionsItemSelected(MenuItem item) {
        if (item.getItemId() == MENU_START_OVER) {
            confirmStartOver();
            return true;
        }
        if (item.getItemId() != MENU_QUIT) {
            return super.onOptionsItemSelected(item);
        }
        startService(new Intent(this, NodeService.class).setAction(NodeService.ACTION_QUIT));
        finishAndRemoveTask();
        return true;
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

    private void confirmStartOver() {
        NodeState s = NodeService.state;
        AlertDialog dialog = new AlertDialog.Builder(this)
                .setTitle(StatusText.resetTitle(s))
                .setMessage(StatusText.resetMessage(s))
                .setNegativeButton("Cancel", null)
                .setPositiveButton(s.inNetwork || s.excluded ? "Leave and reset" : "Reset", (d, w) -> startOver())
                .create();
        dialog.show();
        dialog.getButton(AlertDialog.BUTTON_POSITIVE).setTextColor(0xFFC62828);
    }

    /** Stops the tunnel and the node like Quit, then erases the app data; the system closes the process. */
    private void startOver() {
        startService(new Intent(this, NodeService.class).setAction(NodeService.ACTION_QUIT));
        button.postDelayed(() -> {
            ActivityManager am = (ActivityManager) getSystemService(ACTIVITY_SERVICE);
            am.clearApplicationUserData();
        }, WIPE_DELAY_MS);
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        takeInvite(intent);
        loadPage();
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
        if (req == REQ_VPN && result == RESULT_OK) {
            Prefs.setVpnWanted(this, true);
            startService(new Intent(this, TunnelService.class));
        }
    }
}
