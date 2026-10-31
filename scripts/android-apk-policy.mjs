export function verifyAndroidApkPolicy(permissions, manifest, variant) {
  if (!permissions.includes("uses-permission: name='android.permission.CAMERA'")) {
    throw new Error('Camera permission declaration is missing');
  }
  const allowed = new Set(['android.permission.CAMERA', 'android.permission.INTERNET']);
  if (!permissions.includes("uses-permission: name='android.permission.INTERNET'")) {
    throw new Error('Internet permission for manual update checks is missing');
  }
  for (const match of permissions.matchAll(/uses-permission: name='([^']+)'/g)) {
    // AndroidX adds an app-scoped signature permission for private receivers.
    if (!allowed.has(match[1]) && match[1] !== 'io.github.shelfledger.app.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION') {
      throw new Error(`Unexpected permission: ${match[1]}`);
    }
  }
  if (!/:allowBackup[^\n]*=false\b/.test(manifest)) {
    throw new Error('Android cloud backup must be disabled');
  }
  if (variant === 'release' && !/:usesCleartextTraffic[^\n]*=false\b/.test(manifest)) {
    throw new Error('Release builds must disable cleartext traffic');
  }
}
