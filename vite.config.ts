import { defineConfig } from 'vite';
import * as path from 'path';
import { nlsPlugin } from './scripts/vite-plugin-nls';

function quietMissingSourceMaps() {
  const skip = [/\/vscode-textmate\/.*\.js\.map$/];
  return {
    name: 'sidex-quiet-missing-source-maps',
    configureServer(server: import('vite').ViteDevServer) {
      server.middlewares.use((req, res, next) => {
        const url = req.url ?? '';
        if (skip.some((re) => re.test(url))) {
          res.statusCode = 204;
          res.end();
          return;
        }
        next();
      });
    },
  };
}

/**
 * Proxy VS Code Gallery API requests to OpenVSX API.
 * Translates Microsoft Marketplace POST queries to OpenVSX GET queries.
 */
function galleryProxy() {
  return {
    name: 'sidex-gallery-proxy',
    configureServer(server: import('vite').ViteDevServer) {
      server.middlewares.use(async (req, res, next) => {
        // Only intercept VS Code Gallery API extensionquery requests
        if (!req.url?.startsWith('/api/gallery/extensionquery')) {
          return next();
        }

        // Collect request body
        let body = '';
        req.on('data', (chunk: string) => (body += chunk));
        req.on('end', async () => {
          try {
            const query = JSON.parse(body);
            const filter = query.filters?.[0];
            const criteria = filter?.criteria || [];
            const searchText = criteria.find((c: any) => c.filterType === 10)?.value || '';
            const category = criteria.find((c: any) => c.filterType === 5)?.value;
            const extensionIds = criteria
              .filter((c: any) => c.filterType === 4)
              .map((c: any) => c.value);
            const pageSize = filter?.pageSize || 20;
            const pageNumber = filter?.pageNumber || 0;
            const sortBy = filter?.sortBy || 0; // 0=relevance, 4=downloads, 6=rating

            let openvsxUrl = 'https://open-vsx.org/api/-/search';
            const params = new URLSearchParams();

            if (extensionIds.length > 0) {
              // Query by extension IDs
              const results: any[] = [];
              for (const extId of extensionIds) {
                const [namespace, name] = extId.split('.');
                try {
                  const resp = await fetch(
                    `https://open-vsx.org/api/${namespace}/${name}/latest`,
                  );
                  if (resp.ok) {
                    const data = await resp.json();
                    results.push(convertOpenVsxToGallery(data));
                  }
                } catch {
                  // Skip unavailable extensions
                }
              }
              const response = {
                results: [
                  {
                    extensions: results,
                    resultMetadata: [
                      {
                        metadataType: 'ResultCount',
                        metadataItems: [{ name: 'TotalCount', count: results.length }],
                      },
                    ],
                  },
                ],
              };
              res.setHeader('Content-Type', 'application/json');
              res.statusCode = 200;
              res.end(JSON.stringify(response));
              return;
            }

            if (searchText) {
              params.set('query', searchText);
            }
            params.set('offset', String(pageNumber * pageSize));
            params.set('size', String(pageSize));

            if (category) {
              params.set('category', category);
            }

            // Map VS Code sort to OpenVSX sort
            const sortMap: Record<number, string> = {
              4: 'downloadCount',
              6: 'averageRating',
              10: 'timestamp',
            };
            const openvsxSort = sortMap[sortBy];
            if (openvsxSort) {
              params.set('sortBy', openvsxSort);
            }

            const fetchUrl = `${openvsxUrl}?${params.toString()}`;
            const openvsxResp = await fetch(fetchUrl);

            if (!openvsxResp.ok) {
              res.statusCode = openvsxResp.status;
              res.end(await openvsxResp.text());
              return;
            }

            const openvsxData = await openvsxResp.json();
            const galleryResults = (openvsxData.extensions || []).map(
              convertOpenVsxToGallery,
            );

            const response = {
              results: [
                {
                  extensions: galleryResults,
                  resultMetadata: [
                    {
                      metadataType: 'ResultCount',
                      metadataItems: [
                        {
                          name: 'TotalCount',
                          count: openvsxData.totalSize || galleryResults.length,
                        },
                      ],
                    },
                  ],
                },
              ],
            };

            res.setHeader('Content-Type', 'application/json');
            res.statusCode = 200;
            res.end(JSON.stringify(response));
          } catch (err: any) {
            console.error('[Gallery Proxy] Error:', err);
            res.statusCode = 500;
            res.end(JSON.stringify({ error: err.message }));
          }
        });
      });
    },
  };
}

/**
 * Convert OpenVSX extension metadata to VS Code Gallery format.
 */
function convertOpenVsxToGallery(openvsx: any) {
  const namespace = openvsx.namespace || openvsx.publisher?.publisherName || 'unknown';
  const name = openvsx.name || 'unknown';
  const version = openvsx.version || '0.0.0';
  const publisher = openvsx.publisher?.displayName || namespace;
  const displayName = openvsx.displayName || name;
  const description = openvsx.shortDescription || openvsx.description || '';
  const downloadCount = openvsx.downloadCount || 0;
  const averageRating = openvsx.averageRating || 0;
  const releaseDate = openvsx.timestamp || new Date().toISOString();
  const lastUpdated = openvsx.timestamp || new Date().toISOString();

  // Extract categories/tags
  const categories = openvsx.categories || [];
  const tags = openvsx.tags || [];

  // Build files/asset URIs using OpenVSX format
  const baseUri = `https://open-vsx.org/api/${namespace}/${name}/${version}`;
  const unpkgBase = `https://open-vsx.org/vscode/unpkg/${namespace}/${name}/${version}`;

  return {
    extensionId: `${namespace}.${name}`,
    extensionName: name,
    displayName,
    flags: 'validated',
    shortDescription: description,
    description,
    versions: [
      {
        version,
        targetPlatform: openvsx.targetPlatform || 'universal',
        properties: [
          { key: 'Microsoft.VisualStudio.Code.Engine', value: openvsx.engines?.vscode || '*' },
          { key: 'Microsoft.VisualStudio.Services.LicenseResult', value: openvsx.license || '' },
        ],
        assetUri: unpkgBase,
        fallbackAssetUri: unpkgBase,
        files: [
          { assetType: 'Manifest', source: `${unpkgBase}/package.json` },
          { assetType: 'Icon', source: openvsx.files?.icon || `${baseUri}/file/icon.png` },
          {
            assetType: 'Details',
            source: openvsx.files?.readme || `${baseUri}/file/README.md`,
          },
          {
            assetType: 'License',
            source: openvsx.files?.license || `${baseUri}/file/LICENSE`,
          },
          {
            assetType: 'Download',
            source: openvsx.files?.download || `${baseUri}/file/${namespace}.${name}-${version}.vsix`,
          },
          {
            assetType: 'Vsix',
            source: openvsx.files?.download || `${baseUri}/file/${namespace}.${name}-${version}.vsix`,
          },
        ],
      },
    ],
    publisher: {
      publisherId: namespace,
      publisherName: namespace,
      displayName: publisher,
      flags: 'verified',
    },
    categories,
    tags,
    installationTargets: [{ targetPlatform: 'universal' }],
    statistics: [
      { statisticName: 'install', value: downloadCount },
      { statisticName: 'averagerating', value: averageRating },
      { statisticName: 'ratingcount', value: openvsx.reviewCount || 0 },
    ],
    releaseDate,
    lastUpdated,
    publishedDate: releaseDate,
  };
}

export default defineConfig({
  clearScreen: false,
  assetsInclude: ['**/*.wasm', '**/*.json', '**/*.tmLanguage.json'],
  publicDir: 'public',
  plugins: [nlsPlugin(), quietMissingSourceMaps(), galleryProxy()],
  server: {
    port: 1420,
    strictPort: true,
    allowedHosts: ['.monkeycode-ai.online'],
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_'],
  resolve: {
    alias: {
      'vs': path.resolve(__dirname, 'src/vs'),
    },
  },
  build: {
    target: ['es2022', 'chrome100', 'safari15'],
    minify: 'esbuild',
    sourcemap: true,
    cssCodeSplit: true,
    chunkSizeWarningLimit: 5000,
    rollupOptions: {
      input: {
        index: path.resolve(__dirname, 'index.html'),
        textMateWorker: path.resolve(__dirname, 'src/vs/workbench/services/textMate/browser/backgroundTokenization/worker/textMateTokenizationWorker.workerMain.ts'),
        editorWorker: path.resolve(__dirname, 'src/vs/editor/common/services/editorWebWorkerMain.ts'),
        extensionHostWorker: path.resolve(__dirname, 'src/vs/workbench/api/worker/extensionHostWorkerMain.ts'),
        outputLinkComputerMain: path.resolve(__dirname, 'src/vs/workbench/contrib/output/common/outputLinkComputerMain.ts'),
      },
      output: {
        entryFileNames: (chunkInfo) => {
          if (chunkInfo.name === 'editorWorker') {
            return 'assets/editorWorker.js';
          }
          if (chunkInfo.name === 'textMateWorker') {
            return 'assets/textMateWorker.js';
          }
          if (chunkInfo.name === 'extensionHostWorker') {
            return 'assets/extensionHostWorker.js';
          }
          if (chunkInfo.name === 'outputLinkComputerMain') {
            return 'assets/outputLinkComputerMain.js';
          }
          return 'assets/[name]-[hash].js';
        },
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: (assetInfo) => {
          if ((assetInfo.name ?? '').endsWith('.ts')) {
            const base = (assetInfo.name ?? 'asset').slice(0, -3);
            return `assets/${base}-[hash].js`;
          }
          return 'assets/[name]-[hash][extname]';
        },
        manualChunks(id, { getModuleInfo }) {
          const isWorkerDep = (moduleId: string, visited = new Set<string>()): boolean => {
            if (visited.has(moduleId)) return false;
            visited.add(moduleId);
            const info = getModuleInfo(moduleId);
            if (!info) return false;
            if (info.isEntry && (moduleId.includes('WorkerMain') || moduleId.includes('workerMain'))) {
              return true;
            }
            for (const importer of info.importers) {
              if (isWorkerDep(importer, visited)) return true;
            }
            return false;
          };

          if (isWorkerDep(id)) {
            return undefined;
          }

          if (id.endsWith('/vs/nls.ts') || id.endsWith('/vs/nls.js')) {
            return 'nls';
          }
          if (
            id.includes('/vs/base/') ||
            id.endsWith('/vs/amdX.ts') || id.endsWith('/vs/amdX.js') ||
            id.endsWith('/vs/sidex-bridge.ts') || id.endsWith('/vs/sidex-bridge.js') ||
            id.includes('xterm') || id.includes('/terminal/') ||
            (id.includes('/vs/editor/') && !id.includes('/workbench/')) ||
            id.includes('/vs/platform/')
          ) {
            return 'core';
          }
        },
      },
    },
  },
  optimizeDeps: {
    include: ['vscode-textmate', 'vscode-oniguruma'],
    exclude: ['@tauri-apps/api'],
  },
  worker: {
    format: 'es',
    plugins: () => [],
    rollupOptions: {
      input: {
        outputLinkComputerMain: path.resolve(__dirname, 'src/vs/workbench/contrib/output/common/outputLinkComputerMain.ts'),
      },
      output: {
        entryFileNames: 'assets/outputLinkComputerMain.js',
        chunkFileNames: 'workers/[name]-[hash].js',
      },
    },
  },
});

