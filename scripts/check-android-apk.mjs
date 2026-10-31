import { verifyAndroidApkPolicy } from './android-apk-policy.mjs';
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
const manifest = execFileSync(aapt, ['dump', 'xmltree', apk, '--file', 'AndroidManifest.xml'], { encoding: 'utf8' });
verifyAndroidApkPolicy(permissions, manifest, variant);
console.log(`Verified ${variant} APK permissions and disabled OS backup.`);
