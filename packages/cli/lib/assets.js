'use strict';

// Vendored asset management. First citizen: the swagger-ui zip that Dockerfiles
// and cargo builds reference offline via SWAGGER_UI_DOWNLOAD_URL (metadata
// absorbed verbatim from the retired scripts/download-swagger-ui.js).

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const https = require('node:https');
const { GuardError, ExternalError } = require('./errors.js');

const SWAGGER = {
  VERSION: 'v5.17.14',
  ZIP_NAME: 'swagger-ui-v5.17.14.zip',
  REMOTE_URL: 'https://gh-proxy.org/https://github.com/swagger-api/swagger-ui/archive/refs/tags/v5.17.14.zip',
  SHA256: '481244d0812097b11fbaeef79f71d942b171617f9c9f9514e63acbe13e71ccdc',
};

function assetPath(root) {
  return path.join(root, 'infrastructure', 'vendor', SWAGGER.ZIP_NAME);
}

function verifySha256(buffer, expectedHex) {
  return crypto.createHash('sha256').update(buffer).digest('hex') === expectedHex;
}

function httpsGetBuffer(url) {
  return new Promise((resolve, reject) => {
    https.get(url, (res) => {
      if (res.statusCode !== 200) {
        res.resume();
        reject(new ExternalError(`download failed: HTTP ${res.statusCode}`, { url }));
        return;
      }
      const chunks = [];
      res.on('data', (chunk) => chunks.push(chunk));
      res.on('end', () => resolve(Buffer.concat(chunks)));
      res.on('error', (err) => reject(new ExternalError(`download failed: ${err.message}`, { url })));
    }).on('error', (err) => reject(new ExternalError(`download failed: ${err.message}`, { url })));
  });
}

// mode 'ensure': download only when missing; 'check': CI guard; 'force': re-download.
// expectedSha256 is injectable for tests; production pins SWAGGER.SHA256.
async function ensureAsset({ root, mode = 'ensure', fetchImpl = httpsGetBuffer, fs: nodeFs = fs, expectedSha256 = SWAGGER.SHA256 } = {}) {
  const dest = assetPath(root);
  const exists = nodeFs.existsSync(dest);

  if (mode === 'check') {
    if (!exists) throw new GuardError('ASSET_MISSING', `asset missing: ${dest} — run coder assets download`, { dest });
    return { path: dest, existed: true, downloaded: false, sha256: null };
  }
  if (mode === 'ensure' && exists) {
    return { path: dest, existed: true, downloaded: false, sha256: null };
  }

  const buffer = await fetchImpl(SWAGGER.REMOTE_URL);
  if (!verifySha256(buffer, expectedSha256)) {
    throw new GuardError('ASSET_SHA_MISMATCH', `sha256 mismatch for ${SWAGGER.ZIP_NAME} — refusing to write`, { dest });
  }
  nodeFs.mkdirSync(path.dirname(dest), { recursive: true });
  nodeFs.writeFileSync(dest, buffer);
  return { path: dest, existed: exists, downloaded: true, sha256: SWAGGER.SHA256 };
}

module.exports = { SWAGGER, assetPath, verifySha256, ensureAsset };
