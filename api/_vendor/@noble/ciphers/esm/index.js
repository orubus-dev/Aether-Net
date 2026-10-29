/**
 * Audited & minimal JS implementation of Salsa20, ChaCha and AES. Check out individual modules.
 * @example
```js
import { gcm, siv } from './aes.js';
import { xsalsa20poly1305 } from './salsa.js';
import { secretbox } from './salsa.js'; // == xsalsa20poly1305
import { chacha20poly1305, xchacha20poly1305 } from './chacha.js';

// Unauthenticated encryption: make sure to use HMAC or similar
import { ctr, cfb, cbc, ecb } from './aes.js';
import { salsa20, xsalsa20 } from './salsa.js';
import { chacha20, xchacha20, chacha8, chacha12 } from './chacha.js';

// KW
import { aeskw, aeskwp } from './aes.js';

// Utilities
import { bytesToHex, hexToBytes, bytesToUtf8, utf8ToBytes } from './utils.js';
import { managedNonce, randomBytes } from './webcrypto.js';
import { poly1305 } from './_poly1305.js';
import { ghash, polyval } from './_polyval.js';
```
 * @module
 */
throw new Error('root module cannot be imported: import submodules instead. Check out README');
export {};
//# sourceMappingURL=index.js.map