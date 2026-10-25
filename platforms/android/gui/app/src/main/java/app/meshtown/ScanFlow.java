package app.meshtown;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.ClipData;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.provider.MediaStore;
import android.widget.Toast;
import java.io.File;
import java.util.function.Consumer;

/** "Scan QR": the system camera takes a photo into {@link PhotoSlot}, the photo is decoded off the UI thread. */
final class ScanFlow {
    static final int REQ_SCAN = 2;
    private static final int MAX_SIDE = 2048;
    private final Activity activity;
    private final Consumer<String> onInvite;

    ScanFlow(Activity activity, Consumer<String> onInvite) {
        this.activity = activity;
        this.onInvite = onInvite;
    }

    void start() {
        PhotoSlot.file(activity).delete();
        Intent shoot = new Intent(MediaStore.ACTION_IMAGE_CAPTURE)
                .putExtra(MediaStore.EXTRA_OUTPUT, PhotoSlot.uri())
                .addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION | Intent.FLAG_GRANT_READ_URI_PERMISSION);
        shoot.setClipData(ClipData.newRawUri("", PhotoSlot.uri()));
        try {
            activity.startActivityForResult(shoot, REQ_SCAN);
        } catch (ActivityNotFoundException | SecurityException e) {
            tell(InviteScan.Outcome.UNAVAILABLE);
        }
    }

    void onResult(int resultCode) {
        if (resultCode != Activity.RESULT_OK) {
            return;
        }
        File photo = PhotoSlot.file(activity);
        new Thread(() -> {
            String decoded = decodeFile(photo);
            photo.delete();
            activity.runOnUiThread(() -> finish(decoded));
        }).start();
    }

    private void finish(String decoded) {
        InviteScan.Outcome outcome = InviteScan.outcome(true, decoded);
        if (outcome == InviteScan.Outcome.INVITE) {
            onInvite.accept(InviteScan.inviteOf(decoded));
            return;
        }
        tell(outcome);
    }

    /** The code in the photo, or null when the file is missing, empty, unreadable or holds none. */
    private static String decodeFile(File photo) {
        if (!photo.isFile() || photo.length() == 0) {
            return null;
        }
        try {
            Bitmap bitmap = BitmapFactory.decodeFile(photo.getPath(), sampling(photo));
            int[] pixels = new int[bitmap.getWidth() * bitmap.getHeight()];
            bitmap.getPixels(pixels, 0, bitmap.getWidth(), 0, 0, bitmap.getWidth(), bitmap.getHeight());
            return QrPhoto.decode(pixels, bitmap.getWidth(), bitmap.getHeight());
        } catch (Throwable e) {
            return null;
        }
    }

    private static BitmapFactory.Options sampling(File photo) {
        BitmapFactory.Options bounds = new BitmapFactory.Options();
        bounds.inJustDecodeBounds = true;
        BitmapFactory.decodeFile(photo.getPath(), bounds);
        BitmapFactory.Options options = new BitmapFactory.Options();
        options.inSampleSize = 1;
        while (Math.max(bounds.outWidth, bounds.outHeight) / options.inSampleSize > MAX_SIDE) {
            options.inSampleSize *= 2;
        }
        return options;
    }

    private void tell(InviteScan.Outcome outcome) {
        String text = InviteScan.message(outcome);
        if (text != null) {
            Toast.makeText(activity, text, Toast.LENGTH_LONG).show();
        }
    }
}
