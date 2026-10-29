"use strict";
/**
 * Audited & minimal JS implementation of Salsa20, ChaCha and AES. Check out individual modules.
 * @example
```js
import { gcm, siv } from './esm/aes.js';
import { xsalsa20poly1305 } from './esm/salsa.js';
import { secretbox } from './esm/salsa.js'; // == xsalsa20poly1305
import { chacha20poly1305, xchacha20poly1305 } from './esm/chacha.js';

// Unauthenticated encryption: make sure to use HMAC or similar
import { ctr, cfb, cbc, ecb } from './esm/aes.js';
import { salsa20, xsalsa20 } from './esm/salsa.js';
import { chacha20, xchacha20, chacha8, chacha12 } from './esm/chacha.js';

// KW
import { aeskw, aeskwp } from './esm/aes.js';

// Utilities
import { bytesToHex, hexToBytes, bytesToUtf8, utf8ToBytes } from './esm/utils.js';
import { managedNonce, randomBytes } from './esm/webcrypto.js';
import { poly1305 } from './esm/_poly1305.js';
import { ghash, polyval } from './esm/_polyval.js';
```
 * @module
 */
throw new Error('root module cannot be imported: import submodules instead. Check out README');
//# sourceMappingURL=index.js.map