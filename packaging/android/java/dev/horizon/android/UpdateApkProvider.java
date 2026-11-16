package dev.horizon.android;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;

import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;

/** Read-only access to downloaded update APKs for Android's package installer. */
public final class UpdateApkProvider extends ContentProvider {
    private File updateFile(Uri uri) throws FileNotFoundException {
        if (uri == null || uri.getPathSegments().size() != 2
                || !"updates".equals(uri.getPathSegments().get(0))) {
            throw new FileNotFoundException("Invalid update APK path");
        }
        File root = new File(getContext().getFilesDir(), "updates");
        File file = new File(root, uri.getPathSegments().get(1));
        try {
            String rootPath = root.getCanonicalPath() + File.separator;
            String filePath = file.getCanonicalPath();
            if (!filePath.startsWith(rootPath) || !file.isFile()
                    || !file.getName().toLowerCase(java.util.Locale.ROOT).endsWith(".apk")) {
                throw new FileNotFoundException("Update APK is missing or invalid");
            }
        } catch (IOException e) {
            throw new FileNotFoundException(e.getMessage());
        }
        return file;
    }

    @Override public boolean onCreate() { return true; }

    @Override public String getType(Uri uri) {
        return "application/vnd.android.package-archive";
    }

    @Override public Cursor query(Uri uri, String[] projection, String selection,
            String[] selectionArgs, String sortOrder) {
        try {
            File file = updateFile(uri);
            MatrixCursor cursor = new MatrixCursor(new String[] {
                    OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE
            });
            cursor.addRow(new Object[] {file.getName(), file.length()});
            return cursor;
        } catch (FileNotFoundException e) {
            return null;
        }
    }

    @Override public ParcelFileDescriptor openFile(Uri uri, String mode)
            throws FileNotFoundException {
        if (!"r".equals(mode)) throw new FileNotFoundException("Read-only provider");
        return ParcelFileDescriptor.open(updateFile(uri), ParcelFileDescriptor.MODE_READ_ONLY);
    }

    @Override public Uri insert(Uri uri, ContentValues values) { return null; }
    @Override public int delete(Uri uri, String selection, String[] selectionArgs) { return 0; }
    @Override public int update(Uri uri, ContentValues values, String selection,
            String[] selectionArgs) { return 0; }
}
