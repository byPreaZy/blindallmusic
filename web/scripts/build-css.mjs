// Trunk pre_build hook : `npm run css` through the system shell so it works
// on Windows (npm.cmd) and Linux alike.
import { execSync } from 'node:child_process';

execSync('npm run css', { stdio: 'inherit' });
