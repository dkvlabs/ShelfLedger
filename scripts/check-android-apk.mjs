import { execFileSync } from 'node:child_process';
import { join } from 'node:path';

const [apk, variant] = process.argv.slice(2);
if (!apk || !['debug', 'release'].includes(variant)) {
  throw new Error('Usage: node scripts/check-android-apk.mjs <apk> <debug|release>');
}
const sdk = process.env.ANDROID_HOME;
if (!sdk) throw new Error('ANDROID_HOME is required');
const aapt = join(sdk, 'build-tools', '36.0.0', process.platform === 'win32' ? 'aapt2.exe' : 'aapt2');
const permissions = execFileSync(aapt, ['dump', 'permissions', apk], { encoding: 'utf8' });
if (!permissions.includes("uses-permission: name='android.permission.CAMERA'")) {
  throw new Error('Camera permission declaration is missing');
}
const allowed = new Set(['android.permission.CAMERA']);
if (variant === 'debug') allowed.add('android.permission.INTERNET');
for (const match of permissions.matchAll(/uses-permission: name='([^']+)'/g)) {
  // AndroidX adds an app-scoped signature permission for private receivers.
  if (!allowed.has(match[1]) && match[1] !== 'io.github.shelfledger.app.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION') {
    throw new Error(`Unexpected permission: ${match[1]}`);
  }
}
const manifest = execFileSync(aapt, ['dump', 'xmltree', apk, '--file', 'AndroidManifest.xml'], { encoding: 'utf8' });
if (!/:allowBackup[^\n]*=false\b/.test(manifest)) {
  throw new Error('Android cloud backup must be disabled');
}
if (variant === 'release' && !/:usesCleartextTraffic[^\n]*=false\b/.test(manifest)) {
  throw new Error('Release builds must disable cleartext traffic');
}
console.log(`Verified ${variant} APK permissions and disabled OS backup.`);
