import * as assert from 'assert';

interface GitGraphQueryParams {
    filePath: string;
    commit: string;
    repo: string;
    exists: boolean;
}

function decodeGitGraphQuery(resource: { scheme: string; query: string | null; path: string | null }): GitGraphQueryParams | null {
    let base64Data: string | null = null;

    if (resource.query) {
        base64Data = resource.query;
    }

    if (!base64Data && resource.path && resource.path.includes('?')) {
        const parts = resource.path.split('?');
        if (parts.length > 1) {
            base64Data = parts[1];
        }
    }

    if (!base64Data) {
        const uriString = `${resource.scheme}:${resource.path || ''}${resource.query ? '?' + resource.query : ''}`;
        if (uriString.includes('?')) {
            const parts = uriString.split('?');
            if (parts.length > 1) {
                base64Data = parts[1];
            }
        }
    }

    if (!base64Data) {
        return null;
    }

    try {
        let decoded: string | null = null;

        try {
            const urlDecoded = decodeURIComponent(base64Data);
            const binaryString = atob(urlDecoded);
            const bytes = new Uint8Array(binaryString.length);
            for (let i = 0; i < binaryString.length; i++) {
                bytes[i] = binaryString.charCodeAt(i);
            }
            decoded = new TextDecoder('utf-8').decode(bytes);
        } catch {
            try {
                const binaryString = atob(base64Data);
                const bytes = new Uint8Array(binaryString.length);
                for (let i = 0; i < binaryString.length; i++) {
                    bytes[i] = binaryString.charCodeAt(i);
                }
                decoded = new TextDecoder('utf-8').decode(bytes);
            } catch {
                try {
                    const normalized = base64Data.replace(/-/g, '+').replace(/_/g, '/');
                    const withPadding = normalized.padEnd(normalized.length + (4 - normalized.length % 4) % 4, '=');
                    const binaryString = atob(withPadding);
                    const bytes = new Uint8Array(binaryString.length);
                    for (let i = 0; i < binaryString.length; i++) {
                        bytes[i] = binaryString.charCodeAt(i);
                    }
                    decoded = new TextDecoder('utf-8').decode(bytes);
                } catch {
                    try {
                        const urlDecoded = decodeURIComponent(base64Data);
                        decoded = urlDecoded;
                    } catch {
                        return null;
                    }
                }
            }
        }

        if (decoded === null) {
            return null;
        }

        const result = JSON.parse(decoded);
        return result as GitGraphQueryParams;
    } catch {
        return null;
    }
}

describe('decodeGitGraphQuery', function () {
    describe('base64 decoding strategies', function () {
        it('should decode URL-encoded base64 with UTF-8 characters', function () {
            const testData = {
                filePath: 'docs/中文测试文件.md',
                commit: 'abc123',
                repo: '/workspace/test',
                exists: true
            };
            const jsonString = JSON.stringify(testData);
            const bytes = new TextEncoder().encode(jsonString);
            const base64Encoded = btoa(String.fromCharCode(...bytes));
            const urlEncoded = encodeURIComponent(base64Encoded);

            const result = decodeGitGraphQuery({
                scheme: 'git-graph',
                query: urlEncoded,
                path: 'file.md'
            });

            assert.strictEqual(result?.filePath, testData.filePath);
            assert.strictEqual(result?.commit, testData.commit);
            assert.strictEqual(result?.repo, testData.repo);
            assert.strictEqual(result?.exists, testData.exists);
        });

        it('should decode plain base64 without URL encoding', function () {
            const testData = {
                filePath: 'src/test.ts',
                commit: 'def456',
                repo: '/workspace/test',
                exists: true
            };
            const jsonString = JSON.stringify(testData);
            const bytes = new TextEncoder().encode(jsonString);
            const base64Encoded = btoa(String.fromCharCode(...bytes));

            const result = decodeGitGraphQuery({
                scheme: 'git-graph',
                query: base64Encoded,
                path: 'file.ts'
            });

            assert.strictEqual(result?.filePath, testData.filePath);
            assert.strictEqual(result?.commit, testData.commit);
        });

        it('should decode URL-safe base64 format', function () {
            const testData = {
                filePath: 'path/with+special_chars.js',
                commit: 'ghi789',
                repo: '/workspace/test',
                exists: false
            };
            const jsonString = JSON.stringify(testData);
            const bytes = new TextEncoder().encode(jsonString);
            let base64Encoded = btoa(String.fromCharCode(...bytes));
            const urlSafeEncoded = base64Encoded.replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');

            const result = decodeGitGraphQuery({
                scheme: 'git-graph',
                query: urlSafeEncoded,
                path: 'file.js'
            });

            assert.strictEqual(result?.filePath, testData.filePath);
            assert.strictEqual(result?.exists, testData.exists);
        });

        it('should handle missing query parameter', function () {
            const result = decodeGitGraphQuery({
                scheme: 'git-graph',
                query: null,
                path: 'file.md'
            });

            assert.strictEqual(result, null);
        });

        it('should handle invalid base64 data', function () {
            const result = decodeGitGraphQuery({
                scheme: 'git-graph',
                query: 'invalid-base64!!!',
                path: 'file.md'
            });

            assert.strictEqual(result, null);
        });

        it('should handle empty query parameter', function () {
            const result = decodeGitGraphQuery({
                scheme: 'git-graph',
                query: '',
                path: 'file.md'
            });

            assert.strictEqual(result, null);
        });

        it('should decode base64 with partial padding', function () {
            const testData = {
                filePath: 'short.txt',
                commit: 'jkl012',
                repo: '/tmp',
                exists: true
            };
            const jsonString = JSON.stringify(testData);
            const bytes = new TextEncoder().encode(jsonString);
            let base64Encoded = btoa(String.fromCharCode(...bytes));

            const withoutPadding = base64Encoded.replace(/=+$/, '');

            const result = decodeGitGraphQuery({
                scheme: 'git-graph',
                query: withoutPadding,
                path: 'file.txt'
            });

            assert.strictEqual(result?.filePath, testData.filePath);
        });

        it('should extract query from path if query is missing', function () {
            const testData = {
                filePath: 'src/index.ts',
                commit: 'mno345',
                repo: '/workspace',
                exists: true
            };
            const jsonString = JSON.stringify(testData);
            const bytes = new TextEncoder().encode(jsonString);
            const base64Encoded = btoa(String.fromCharCode(...bytes));

            const result = decodeGitGraphQuery({
                scheme: 'git-graph',
                query: null,
                path: `file.ts?${base64Encoded}`
            });

            assert.strictEqual(result?.filePath, testData.filePath);
            assert.strictEqual(result?.commit, testData.commit);
        });

        it('should handle complex nested paths with UTF-8', function () {
            const testData = {
                filePath: '深度嵌套/路径/中文文件名/测试文件.py',
                commit: 'pqr678',
                repo: '/home/user/projects/test',
                exists: true
            };
            const jsonString = JSON.stringify(testData);
            const bytes = new TextEncoder().encode(jsonString);
            const base64Encoded = btoa(String.fromCharCode(...bytes));
            const urlEncoded = encodeURIComponent(base64Encoded);

            const result = decodeGitGraphQuery({
                scheme: 'git-graph',
                query: urlEncoded,
                path: 'file.py'
            });

            assert.strictEqual(result?.filePath, testData.filePath);
            assert.strictEqual(result?.repo, testData.repo);
        });

        it('should handle commit with caret suffix', function () {
            const testData = {
                filePath: 'src/main.ts',
                commit: 'stu901^',
                repo: '/workspace/test',
                exists: true
            };
            const jsonString = JSON.stringify(testData);
            const bytes = new TextEncoder().encode(jsonString);
            const base64Encoded = btoa(String.fromCharCode(...bytes));

            const result = decodeGitGraphQuery({
                scheme: 'git-graph',
                query: base64Encoded,
                path: 'file.ts'
            });

            assert.strictEqual(result?.commit, testData.commit);
        });
    });

    describe('Git Graph URI format', function () {
        it('should parse typical Git Graph URI format', function () {
            const testData = {
                filePath: 'src/web/config/http.config.ts',
                commit: '426fbebf6f7690efba4478e8fb37879a1287c652^',
                repo: 'c:/Users/liqifeng/Desktop/live',
                exists: true
            };
            const jsonString = JSON.stringify(testData);
            const bytes = new TextEncoder().encode(jsonString);
            const base64Encoded = btoa(String.fromCharCode(...bytes));
            const urlEncoded = encodeURIComponent(base64Encoded);

            const result = decodeGitGraphQuery({
                scheme: 'git-graph',
                query: urlEncoded,
                path: 'file.config.ts'
            });

            assert.strictEqual(result?.filePath, testData.filePath);
            assert.strictEqual(result?.commit, testData.commit);
            assert.strictEqual(result?.repo, testData.repo);
        });

        it('should parse URI with special characters in path', function () {
            const testData = {
                filePath: 'docs/PC助手支持匿名设置/PC助手支持匿名设置-需求文档.md',
                commit: '028c744b3b408f1e3efea18d17256189b146e48a^',
                repo: 'c:/Users/liqifeng/Desktop/live',
                exists: true
            };
            const jsonString = JSON.stringify(testData);
            const bytes = new TextEncoder().encode(jsonString);
            const base64Encoded = btoa(String.fromCharCode(...bytes));
            const urlEncoded = encodeURIComponent(base64Encoded);

            const result = decodeGitGraphQuery({
                scheme: 'git-graph',
                query: urlEncoded,
                path: 'file.md'
            });

            assert.strictEqual(result?.filePath, testData.filePath);
            assert.strictEqual(result?.commit, testData.commit);
        });
    });
});

if (typeof module !== 'undefined' && module.exports) {
    module.exports = { decodeGitGraphQuery };
}